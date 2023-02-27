use crate::capture::{AcquireFrameError, ScreenDuplicator};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use webrtc::{
    ice_transport::ice_connection_state::RTCIceConnectionState,
    rtcp::{
        self,
        payload_feedbacks::{
            full_intra_request::FullIntraRequest, picture_loss_indication::PictureLossIndication,
        },
    },
    rtp::header::Header,
    rtp_transceiver::RTCRtpTransceiver,
    track::track_local::track_local_static_rtp::TrackLocalStaticRTP,
};
use webrtc_helper::{
    codecs::H264SampleSender, interceptor::twcc::TwccBandwidthEstimate, peer::IceConnectionState,
};
use windows::Win32::System::Performance::QueryPerformanceFrequency;

const RTP_MTU: usize = 1200;
const RTCP_MAX_MTU: usize = 1500;
const MIN_BITRATE_BPS: u32 = 64_000;
const MAX_BITRATE_BPS: u32 = 100_000_000;

#[derive(Debug, PartialEq, Eq, Clone)]
enum RtcpEvent {
    Pli,
    Fir,
}

struct NvidiaEncoderInput {
    screen_duplicator: ScreenDuplicator,
    input: nvenc::EncoderInput<nvenc::DirectX11Device>,
    bandwidth_estimate: TwccBandwidthEstimate,
    frame_rate_num: u32,
    frame_rate_den: u32,
    rtcp_rx: UnboundedReceiver<RtcpEvent>,
}

impl NvidiaEncoderInput {
    fn new(
        screen_duplicator: ScreenDuplicator,
        input: nvenc::EncoderInput<nvenc::DirectX11Device>,
        bandwidth_estimate: TwccBandwidthEstimate,
        rtcp_rx: UnboundedReceiver<RtcpEvent>,
    ) -> NvidiaEncoderInput {
        let (frame_rate_num, frame_rate_den) = {
            let display_desc = screen_duplicator.desc();
            (
                display_desc.ModeDesc.RefreshRate.Numerator,
                display_desc.ModeDesc.RefreshRate.Numerator,
            )
        };

        NvidiaEncoderInput {
            screen_duplicator,
            input,
            bandwidth_estimate,
            frame_rate_num,
            frame_rate_den,
            rtcp_rx,
        }
    }

    fn update_bitrate(&mut self) {
        let bitrate = self.bandwidth_estimate.borrow().bits_per_sec() as u32;
        let bitrate = bitrate.clamp(MIN_BITRATE_BPS, MAX_BITRATE_BPS);
        // Divide first to prevent overflow
        let vbv_buffer_size = bitrate / self.frame_rate_num * self.frame_rate_den;
        if let Err(e) = self
            .input
            .update_average_bitrate(bitrate, Some(vbv_buffer_size))
        {
            log::error!("Error trying to update bitrate: {e}");
        }
    }

    fn encode(&mut self) -> Result<(), nvenc::NvEncError> {
        match self.screen_duplicator.acquire_frame(4294967295u32) {
            Ok((acquired_image, info)) => {
                let timestamp = info.LastPresentTime as u64;
                // Check if image was updated
                if timestamp != 0 {
                    self.input.encode_frame(acquired_image, timestamp)?;
                }
                Ok(())
            }
            Err(e) => match e {
                AcquireFrameError::Retry => Ok(()),
                AcquireFrameError::Unknown => panic!("{:?}", e),
            },
        }
    }
}

struct NvidiaEncoderOutput {
    output: nvenc::EncoderOutput,
    rtp_track: Arc<TrackLocalStaticRTP>,
    payloader: H264SampleSender,
    header: Header,
    clock_rate: u32,
    timer_frequency: u64,
    timestamp: u32,
    prev_timestamp_source: Option<u64>,
}

impl NvidiaEncoderOutput {
    fn new(
        output: nvenc::EncoderOutput,
        rtp_track: Arc<TrackLocalStaticRTP>,
        payload_type: u8,
        ssrc: u32,
        clock_rate: u32,
    ) -> NvidiaEncoderOutput {
        let payloader = H264SampleSender::default();
        let timer_frequency = timer_frequency();
        let header = Header {
            version: 2,
            padding: false,
            extension: false,
            marker: false,
            payload_type,
            sequence_number: 0,
            ssrc,
            ..Default::default()
        };

        NvidiaEncoderOutput {
            output,
            rtp_track,
            payloader,
            header,
            clock_rate,
            timer_frequency,
            timestamp: rand::random::<u32>(),
            prev_timestamp_source: None,
        }
    }

    fn write_packets(&mut self, handle: &tokio::runtime::Handle) -> Result<(), nvenc::NvEncError> {
        let encode_result = self.output.wait_for_output(|lock| {
            let slice = unsafe {
                std::slice::from_raw_parts(
                    lock.bitstreamBufferPtr as *const u8,
                    lock.bitstreamSizeInBytes as usize,
                )
            };

            // This conversion is chosen even though it causes the timestamp to be prone to drift
            // because only accurate frame intervals are important.
            if let Some(prev) = self.prev_timestamp_source {
                let delta_source = lock.outputTimeStamp.wrapping_sub(prev);
                let delta =
                    delta_source.wrapping_mul(self.clock_rate as u64) / self.timer_frequency;
                // Accumulates small errors coming from `delta`. Can cause the timestamp to drift
                // from the source's timestamp.
                self.timestamp = self.timestamp.wrapping_add(delta as u32);
            }
            self.prev_timestamp_source = Some(lock.outputTimeStamp);

            self.header.timestamp = self.timestamp;

            // Send the encoded frames
            let write_result = handle.block_on(async {
                self.payloader
                    .send_payload(RTP_MTU - 12, &mut self.header, slice, &*self.rtp_track)
                    .await
            });

            if let Err(e) = write_result {
                log::error!("Error writing RTP: {e}");
            }
        });

        encode_result
    }
}

async fn rtcp_handler(
    transceiver: Arc<RTCRtpTransceiver>,
    mut ice_connection_state: IceConnectionState,
    rtcp_tx: UnboundedSender<RtcpEvent>,
    ssrc: u32,
) {
    if let Some(sender) = transceiver.sender().await {
        let mut buf = vec![0u8; RTCP_MAX_MTU];

        loop {
            tokio::select! {
                _ = ice_connection_state.changed() => {
                    if *ice_connection_state.borrow() != RTCIceConnectionState::Connected {
                        break;
                    }
                }
                read_result = sender.read(&mut buf) => {
                    if let Ok((n, _)) = read_result {
                        let mut raw_data = &buf[..n];
                        if let Ok(packets) = rtcp::packet::unmarshal(&mut raw_data) {
                            for packet in packets {
                                let packet = packet.as_any();
                                if let Some(pli) = packet.downcast_ref::<PictureLossIndication>() {
                                    if pli.media_ssrc == ssrc {
                                        if let Err(e) = rtcp_tx.send(RtcpEvent::Pli) {
                                            log::warn!("Error while sending RtcpEvent: {e}");
                                        }
                                    }
                                } else if let Some(fir) = packet.downcast_ref::<FullIntraRequest>() {
                                    if fir.media_ssrc == ssrc {
                                        if let Err(e) = rtcp_tx.send(RtcpEvent::Fir) {
                                            log::warn!("Error while sending RtcpEvent: {e}");
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }
    let _ = transceiver.stop().await;
    log::info!("RTCP handler exited");
}

pub async fn start_encoder(
    screen_duplicator: ScreenDuplicator,
    input: nvenc::EncoderInput<nvenc::DirectX11Device>,
    output: nvenc::EncoderOutput,
    rtp_track: Arc<TrackLocalStaticRTP>,
    transceiver: Arc<RTCRtpTransceiver>,
    mut ice_connection_state: IceConnectionState,
    bandwidth_estimate: TwccBandwidthEstimate,
    payload_type: u8,
    ssrc: u32,
    clock_rate: u32,
) {
    while *ice_connection_state.borrow() != RTCIceConnectionState::Connected {
        if let Err(_) = ice_connection_state.changed().await {
            log::error!("Peer exited before ICE became connected");
            return;
        }
    }
    // tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let (rtcp_tx, rtcp_rx) = unbounded_channel();

    tokio::spawn(rtcp_handler(
        transceiver,
        ice_connection_state.clone(),
        rtcp_tx,
        ssrc,
    ));

    let mut input = NvidiaEncoderInput::new(screen_duplicator, input, bandwidth_estimate, rtcp_rx);
    let mut output = NvidiaEncoderOutput::new(output, rtp_track, payload_type, ssrc, clock_rate);

    let ice_1 = ice_connection_state;
    let ice_2 = ice_1.clone();

    tokio::spawn(tokio::task::unconstrained(async move {
        // TODO: Frame interval should be configurable and/or signaled in SDP
        let mut interval = tokio::time::interval(std::time::Duration::from_nanos(16_666_667));
        while *ice_1.borrow() == RTCIceConnectionState::Connected {
            // TODO: *Average* frame interval is correct but the min/max is off by a lot
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = input.encode() {
                        log::error!("Error encoding: {e}");
                    }
                }
                msg = input.rtcp_rx.recv() => {
                    match msg {
                        Some(event) => match event {
                            RtcpEvent::Pli => {
                                // FIXME: Properly handle SSRC
                                input.input.force_idr_on_next();
                                log::info!("PLI received");
                            }
                            RtcpEvent::Fir => {
                                // FIXME: Properly handle SSRC and seq nums
                                input.input.force_idr_on_next();
                                log::info!("FIR received");
                            }
                        }
                        None => break,
                    }
                }
                _ = input.bandwidth_estimate.changed() => {
                    input.update_bitrate();
                }
            }
        }
        log::info!("Input thread exited");
    }));

    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        while *ice_2.borrow() == RTCIceConnectionState::Connected {
            if let Err(e) = output.write_packets(&handle) {
                log::error!("Error while waiting for output: {e}");
                break;
            }
        }
        log::info!("Output thread exited");
    });
}

fn timer_frequency() -> u64 {
    let mut timer_frequency = 0;
    unsafe {
        QueryPerformanceFrequency(&mut timer_frequency);
    }
    timer_frequency as u64
}

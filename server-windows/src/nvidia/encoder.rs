use crate::{capture::ScreenDuplicator, payloader::H264Payloader};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
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
use webrtc_helper::{peer::IceConnectionState, util::data_rate::TwccBandwidthEstimate};
use windows::Win32::{
    Graphics::Dxgi::{DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT},
    System::Performance::QueryPerformanceFrequency,
};

const RTP_MTU: usize = 1200;
const RTCP_MAX_MTU: usize = 1500;
const MIN_BITRATE_MBPS: u32 = 10_000;
const MAX_BITRATE_MBPS: u32 = 100_000_000;

#[derive(Debug, PartialEq, Eq, Clone)]
enum RtcpEvent {
    Pli(PictureLossIndication),
    Fir(FullIntraRequest),
}

struct NvidiaEncoderInput {
    screen_duplicator: ScreenDuplicator,
    acquire_timeout_millis: u32,
    input: nvenc::EncoderInput<nvenc::DirectX11Device>,
    bandwidth_estimate: TwccBandwidthEstimate,
    rtcp_rx: UnboundedReceiver<RtcpEvent>,
}

impl NvidiaEncoderInput {
    fn new(
        screen_duplicator: ScreenDuplicator,
        input: nvenc::EncoderInput<nvenc::DirectX11Device>,
        bandwidth_estimate: TwccBandwidthEstimate,
        rtcp_rx: UnboundedReceiver<RtcpEvent>,
    ) -> NvidiaEncoderInput {
        // Half of frame interval to allow processing RTCP in-between
        let acquire_timeout_millis = (screen_duplicator.frame_interval() / 2).as_millis() as u32;
        NvidiaEncoderInput {
            screen_duplicator,
            acquire_timeout_millis,
            input,
            bandwidth_estimate,
            rtcp_rx,
        }
    }

    fn update_bitrate(&mut self) {
        let bitrate = self.bandwidth_estimate.borrow().bits_per_sec() as u32;
        let bitrate = bitrate.clamp(MIN_BITRATE_MBPS, MAX_BITRATE_MBPS);
        if let Err(e) = self.input.update_average_bitrate(bitrate) {
            log::error!("Error trying to update bitrate: {e}");
        }
    }

    fn encode(&mut self) {
        match self
            .screen_duplicator
            .acquire_frame(self.acquire_timeout_millis)
        {
            Ok((acquired_image, info)) => {
                let timestamp = u64::from_ne_bytes(info.LastPresentTime.to_ne_bytes());
                self.input
                    .encode_frame(acquired_image, timestamp, || {
                        self.screen_duplicator.release_frame().unwrap()
                    })
                    .unwrap();
            }
            Err(e) => {
                match e.code() {
                    DXGI_ERROR_WAIT_TIMEOUT => {
                        if let Ok(true) = self.bandwidth_estimate.has_changed() {
                            self.update_bitrate();
                        }

                        match self.rtcp_rx.try_recv() {
                            Ok(RtcpEvent::Pli(_pli)) => {
                                // FIXME: Properly handle SSRC
                                self.input.force_idr_on_next();
                                log::info!("PLI received");
                            }
                            Ok(RtcpEvent::Fir(_fir)) => {
                                // FIXME: Properly handle SSRC and seq nums
                                self.input.force_idr_on_next();
                                log::info!("FIR received");
                            }
                            _ => (), // Ignore errors
                        }
                    }
                    DXGI_ERROR_ACCESS_LOST => {
                        // Reset duplicator then move on to next frame acquisition
                        self.screen_duplicator.reset_output_duplicator().unwrap();
                    }
                    _ => panic!("{}", e),
                }
            }
        }
    }
}

struct NvidiaEncoderOutput {
    output: nvenc::EncoderOutput,
    rtp_track: Arc<TrackLocalStaticRTP>,
    payloader: H264Payloader,
    timer_frequency: u64,
    header: Header,
}

impl NvidiaEncoderOutput {
    fn new(
        output: nvenc::EncoderOutput,
        rtp_track: Arc<TrackLocalStaticRTP>,
        payload_type: u8,
        ssrc: u32,
    ) -> NvidiaEncoderOutput {
        let payloader = H264Payloader::default();
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
            timer_frequency,
            header,
        }
    }

    fn write_packets(&mut self, handle: &tokio::runtime::Handle) {
        let encode_result = self.output.wait_for_output(|lock| {
            let slice = unsafe {
                std::slice::from_raw_parts(
                    lock.bitstreamBufferPtr as *const u8,
                    lock.bitstreamSizeInBytes as usize,
                )
            };

            self.header.timestamp =
                convert_to_90_khz_timestamp(lock.outputTimeStamp, self.timer_frequency);

            // Send the encoded frames
            let write_result = handle.block_on(async {
                self.payloader
                    .write_to_rtp(
                        RTP_MTU - 12,
                        &mut self.header,
                        &bytes::Bytes::copy_from_slice(slice),
                        &*self.rtp_track,
                    )
                    .await
            });

            if let Err(e) = write_result {
                log::error!("Error writing RTP: {e}");
            }
        });

        if let Err(e) = encode_result {
            log::error!("Error while waiting for output: {e}")
        }
    }
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
) {
    log::info!("start_encoder");
    while *ice_connection_state.borrow() != RTCIceConnectionState::Connected {
        if let Err(_) = ice_connection_state.changed().await {
            log::error!("Peer exited before ICE became connected");
            return;
        }
    }

    let (rtcp_tx, rtcp_rx) = unbounded_channel();

    if let Some(sender) = transceiver.sender().await {
        tokio::spawn(async move {
            let mut buf = vec![0u8; RTCP_MAX_MTU];
            while let Ok((n, _)) = sender.read(&mut buf).await {
                let mut raw_data = &buf[..n];
                if let Ok(packets) = rtcp::packet::unmarshal(&mut raw_data) {
                    for packet in packets {
                        let packet = packet.as_any();
                        if let Some(pli) = packet.downcast_ref::<PictureLossIndication>() {
                            if let Err(e) = rtcp_tx.send(RtcpEvent::Pli(pli.clone())) {
                                log::warn!("Error while sending RtcpEvent: {e}");
                            }
                        } else if let Some(fir) = packet.downcast_ref::<FullIntraRequest>() {
                            if let Err(e) = rtcp_tx.send(RtcpEvent::Fir(fir.clone())) {
                                log::warn!("Error while sending RtcpEvent: {e}");
                            }
                        }
                    }
                }
            }
        });
    }

    let mut input = NvidiaEncoderInput::new(screen_duplicator, input, bandwidth_estimate, rtcp_rx);
    let mut output = NvidiaEncoderOutput::new(output, rtp_track, payload_type, ssrc);

    let ice_1 = ice_connection_state;
    let ice_2 = ice_1.clone();

    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        while *ice_1.borrow() == RTCIceConnectionState::Connected {
            handle.block_on(async {
                input.encode();
            });
        }
    });

    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        while *ice_2.borrow() == RTCIceConnectionState::Connected {
            output.write_packets(&handle);
        }
    });
}

fn timer_frequency() -> u64 {
    let mut timer_frequency = 0;
    unsafe {
        QueryPerformanceFrequency(&mut timer_frequency);
    }
    u64::from_ne_bytes(timer_frequency.to_ne_bytes())
}

fn convert_to_90_khz_timestamp(time_stamp: u64, timer_frequency: u64) -> u32 {
    ((time_stamp * 90000) / timer_frequency) as u32
}

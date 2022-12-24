mod builder;
mod payloader;

use crate::capture::ScreenDuplicator;
use tokio::sync::mpsc::{channel, error::TryRecvError, Receiver, Sender};
use webrtc::rtp::{
    packet::Packet,
    sequence::{new_random_sequencer, Sequencer},
};
use webrtc_helper::{encoder::Encoder, util::data_rate::DataRate};
use windows::{
    core::Interface,
    Win32::{
        Graphics::{
            Direct3D11::ID3D11Texture2D,
            Dxgi::{
                Common::DXGI_FORMAT,
                {DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT},
            },
        },
        System::Performance::QueryPerformanceFrequency,
    },
};

const CHANNEL_SIZE: usize = 4;

pub struct NvidiaEncoder {
    output: nvenc::EncoderOutput,
    sender: Sender<DataRate>,
    payload_type: u8,
    ssrc: u32,
    timer_frequency: u64,
    sequencer: Box<dyn Sequencer + Send + Sync>,
    payloader: payloader::H264Payloader,
    packets: Vec<Packet>,
}

impl Encoder for NvidiaEncoder {
    fn packets(&mut self, mtu: usize, data_rate: DataRate) -> &[Packet] {
        if let Err(e) = self.sender.blocking_send(data_rate) {
            log::error!("{e}");
        }
        self.packets.clear();

        let mut timestamp = 0;

        // Send the encoded frames
        if let Err(e) = self.output.wait_for_output(|lock| {
            let slice = unsafe {
                std::slice::from_raw_parts(
                    lock.bitstreamBufferPtr as *const u8,
                    lock.bitstreamSizeInBytes as usize,
                )
            };

            // Convert to 90000 Hz timestamp
            timestamp = ((lock.outputTimeStamp * 90000) / self.timer_frequency) as u32;

            if let Err(e) = self.payloader.payload(
                mtu,
                &bytes::Bytes::copy_from_slice(slice),
                &mut self.packets,
            ) {
                log::error!("{e}");
                panic!("Error in fragmenting NALU");
            }
        }) {
            log::error!("{e}");
            panic!("Error while waiting for output");
        }

        for packet in &mut self.packets {
            let header = &mut packet.header;
            header.version = 2;
            header.padding = false;
            header.extension = false;
            header.marker = false;
            header.payload_type = self.payload_type;
            header.sequence_number = self.sequencer.next_sequence_number();
            header.timestamp = timestamp;
            header.ssrc = self.ssrc;
        }

        if let Some(last) = self.packets.last_mut() {
            last.header.marker = true;
        }

        &self.packets
    }
}

impl NvidiaEncoder {
    pub fn new(
        screen_duplicator: ScreenDuplicator,
        display_formats: Vec<DXGI_FORMAT>,
        input: nvenc::EncoderInput<nvenc::DirectX11Device>,
        output: nvenc::EncoderOutput,
        payload_type: u8,
        ssrc: u32,
    ) -> Self {
        let (tx, rx) = channel::<DataRate>(CHANNEL_SIZE);

        std::thread::spawn(move || {
            NvidiaEncoder::encoder_input_loop(screen_duplicator, display_formats, input, rx);
        });

        let mut timer_frequency = 0;
        unsafe {
            QueryPerformanceFrequency(&mut timer_frequency);
        }
        let timer_frequency = u64::from_ne_bytes(timer_frequency.to_ne_bytes());

        NvidiaEncoder {
            output,
            sender: tx,
            payload_type,
            ssrc,
            timer_frequency,
            sequencer: Box::new(new_random_sequencer()),
            payloader: payloader::H264Payloader::default(),
            packets: Vec::new(),
        }
    }

    fn encoder_input_loop(
        mut screen_duplicator: ScreenDuplicator,
        display_formats: Vec<DXGI_FORMAT>,
        mut input: nvenc::EncoderInput<nvenc::DirectX11Device>,
        mut receiver: Receiver<DataRate>,
    ) {
        let mut bitrate = 0;
        let mut prev_bitrate = 0;

        loop {
            match receiver.try_recv() {
                Ok(data_rate) => {
                    bitrate = data_rate.bits_per_sec() as u32;
                }
                Err(TryRecvError::Empty) => {
                    if prev_bitrate != bitrate {
                        if let Err(e) = input.update_average_bitrate(bitrate) {
                            log::error!("{e}");
                            panic!("Error trying to update bitrate");
                        }
                        prev_bitrate = bitrate;
                    }

                    let (resource, info) = loop {
                        match screen_duplicator.acquire_frame() {
                            Ok(r) => break r,
                            Err(e) => {
                                match e.code() {
                                    DXGI_ERROR_WAIT_TIMEOUT => {
                                        log::info!("AcquireNextFrame timed-out");
                                    }
                                    DXGI_ERROR_ACCESS_LOST => {
                                        // Must call reset_output_duplicator if AccessLost
                                        screen_duplicator
                                            .reset_output_duplicator(&display_formats)
                                            .unwrap();
                                    }
                                    _ => panic!("{}", e),
                                }
                            }
                        }
                    };

                    // `IDXGIResource` to `ID3D11Texture2D` should never fail
                    let acquired_image: ID3D11Texture2D = resource.cast().unwrap();

                    let timestamp = u64::from_ne_bytes(info.LastPresentTime.to_ne_bytes());
                    input
                        .encode_frame(acquired_image, timestamp, || {
                            screen_duplicator.release_frame().unwrap()
                        })
                        .unwrap();
                }
                Err(TryRecvError::Disconnected) => break, // Sender exited; break out of loop
            }
        }
    }
}

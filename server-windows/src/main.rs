mod capture;
mod device;

use nvenc_windows::{Codec, EncoderPreset, TuningInfo};
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT};

use std::fs::File;
use std::io::prelude::*;

fn main() {
    let display_index = 0;
    let formats = vec![windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM];
    const BUF_SIZE: usize = 8;
    const NUM_FRAMES: usize = 120;

    let codec = Codec::H264;
    let preset = EncoderPreset::P7;
    let tuning_info = TuningInfo::UltraLowLatency;

    let device = device::create_d3d11_device().unwrap();
    let mut duplicator =
        capture::ScreenDuplicator::new(device.clone(), display_index, &formats).unwrap();
    let display_desc = duplicator.desc();

    let (mut encoder, encoder_output) = nvenc_windows::create_encoder::<BUF_SIZE>(
        device,
        &display_desc,
        codec,
        preset,
        tuning_info,
    );

    let a = std::thread::spawn(move || {
        let mut i = 0;

        while let Ok(_) = encoder_output.wait_for_output(|lock| {
            println!(
                "{}: {} bytes",
                lock.outputTimeStamp, lock.bitstreamSizeInBytes
            );

            let mut file = File::create(format!("target/dump/{}.h264", i)).unwrap();
            i += 1;

            let slice = unsafe {
                std::slice::from_raw_parts(
                    lock.bitstreamBufferPtr as *const u8,
                    lock.bitstreamSizeInBytes as usize,
                )
            };

            file.write_all(slice).unwrap();
        }) {}
        println!("Exiting");
    });

    {
        let mut file = File::create("target/dump/csd.bin").unwrap();
        let csd = encoder.get_codec_specific_data().unwrap();
        file.write_all(&csd).unwrap();
    }

    for _i in 0..NUM_FRAMES {
        let (resource, info) = loop {
            match duplicator.acquire_frame() {
                Ok(r) => break r,
                Err(e) => {
                    match e.code() {
                        // TODO: Log timeouts
                        DXGI_ERROR_WAIT_TIMEOUT => (),
                        // must call reset_output_duplicator if AccessLost
                        DXGI_ERROR_ACCESS_LOST => {
                            duplicator.reset_output_duplicator(&formats).unwrap();
                        }
                        _ => panic!("{}", e),
                    }
                }
            }
        };

        encoder
            .encode_frame(resource, info.LastPresentTime as u32)
            .unwrap();
        duplicator.release_frame().unwrap();
    }

    std::mem::drop(encoder);
    a.join().unwrap();
}

mod capture;
mod device;

use nvenc::{Codec, EncoderPreset, TuningInfo};
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT};

use std::fs::File;
use std::io::prelude::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

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

    let (mut encoder, encoder_output) = nvenc::create_encoder::<BUF_SIZE>(
        device,
        &display_desc,
        codec,
        preset,
        tuning_info,
    );

    // {
    //     for codec in &encoder.codecs().unwrap() {
    //         println!("{:?}", codec);
    //         println!("    {:?}", &encoder.codec_profiles(*codec));
    //         println!("    {:?}", &encoder.supported_input_formats(*codec));
    //     }

    //     let csd = encoder.get_codec_specific_data().unwrap();
    //     println!("\nSPS:\n{},{:b},{}", csd[5], csd[6], csd[7]);
    //     return;
    // }

    let end = Arc::new(AtomicBool::new(false));
    let eos = end.clone();

    let a = std::thread::spawn(move || {
        // For debugging
        #[allow(unused_variables, unused_mut)]
        let mut i = 0;

        let mut timestamps = Vec::with_capacity(120);

        while !eos.load(Ordering::Acquire) {
            if let Ok(_) = encoder_output.wait_for_output(|lock| {
                let now = timer_counter() as u64;
                let time_delta = now - lock.outputTimeStamp;
                timestamps.push(time_delta);

                println!(
                    "{} - {}: {} bytes",
                    lock.frameIdx, time_delta, lock.bitstreamSizeInBytes
                );

                let mut file = File::create(format!("nvenc/regex/nalus/{}.h264", i)).unwrap();
                i += 1;

                let slice = unsafe {
                    std::slice::from_raw_parts(
                        lock.bitstreamBufferPtr as *const u8,
                        lock.bitstreamSizeInBytes as usize,
                    )
                };

                file.write_all(slice).unwrap();
            }) {}
        }
        let div = timer_frequency() as u64 / 1000000;
        for v in &mut timestamps {
            *v /= div;
        }
        println!("Exiting");
        print_stats(&timestamps);
        println!("\nWithout the first delta:");
        print_stats(&timestamps[1..]);
    });

    {
        let csd = encoder.get_codec_specific_data().unwrap();
        let mut file = File::create("nvenc/regex/nalus/csd.bin").unwrap();
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
            .encode_frame(resource, info.LastPresentTime as u64, || {
                duplicator.release_frame().unwrap()
            })
            .unwrap();
    }

    std::mem::drop(encoder);
    end.store(true, Ordering::Release);
    a.join().unwrap();
}

fn print_stats(deltas: &[u64]) {
    let sum: f64 = deltas.iter().map(|&x| x as f64).sum();
    let ave = sum / deltas.len() as f64;
    let sum_sqdiff: f64 = deltas
        .iter()
        .map(|&x| {
            let diff = x as f64 - ave;
            diff * diff
        })
        .sum();
    let stddev = (sum_sqdiff / deltas.len() as f64).sqrt();
    println!("|Average|{}|", ave);
    println!("|Stddev|{}|", stddev);
}

fn timer_counter() -> i64 {
    let mut now = 0;
    unsafe {
        windows::Win32::System::Performance::QueryPerformanceCounter(&mut now);
        now
    }
}

fn timer_frequency() -> i64 {
    let mut freq = 0;
    unsafe {
        windows::Win32::System::Performance::QueryPerformanceFrequency(&mut freq);
        freq
    }
}

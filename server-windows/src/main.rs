mod capture;
mod device;

use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT};

fn main() {
    let display_index = 0;
    let formats = vec![windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM];
    const NUM_FRAMES: usize = 100;

    let device = device::create_d3d11_device().unwrap();
    let mut duplicator =
        capture::ScreenDuplicator::new(device.clone(), display_index, &formats).unwrap();
    let display_desc = duplicator.desc();

    let (mut encoder_input, encoder_output, frame_sender, copy_complete_receiver) =
        nvenc_windows::create_encoder::<8>(device, &display_desc);

    std::thread::spawn(move || {
        for _i in 0..NUM_FRAMES {
            encoder_input.wait_and_encode_frame().unwrap();
        }
    });

    std::thread::spawn(move || {
        for _i in 0..NUM_FRAMES {
            encoder_output
                .wait_for_output(|lock| {
                    println!("{}: {}", lock.outputTimeStamp, lock.bitstreamSizeInBytes);
                })
                .unwrap();
        }
    });

    for _i in 0..NUM_FRAMES {
        let (resource, _x) = loop {
            match duplicator.acquire_frame() {
                Ok(r) => break r,
                Err(e) => {
                    match e.code() {
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
        frame_sender.send(resource).unwrap();
        copy_complete_receiver.recv().unwrap();
        duplicator.release_frame().unwrap();
    }
}

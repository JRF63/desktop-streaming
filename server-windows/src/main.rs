mod capture;
mod device;

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
            encoder_output.wait_for_output(|lock_bitstream| {
                // println!("{}", lock_bitstream.bitstreamSizeInBytes);
            }).unwrap();
        }
    });

    for _i in 0..NUM_FRAMES {
        println!("loop");
        let (resource, _x) = duplicator.acquire_frame().unwrap();
        frame_sender.send(resource).unwrap();
        copy_complete_receiver.recv().unwrap();
        duplicator.release_frame().unwrap();
    }
}

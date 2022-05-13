mod capture;
mod device;
mod texture_buffer;

fn main() {
    device::create_d3d11_device().unwrap();
    println!("Hello, world!");
}

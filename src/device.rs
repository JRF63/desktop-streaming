use windows::{
    core::{Interface, Result},
    Win32::Graphics::{
        Direct3D::{self, D3D_DRIVER_TYPE_HARDWARE},
        Direct3D11::{self, D3D11CreateDevice, ID3D11Device, ID3D11Multithread, D3D11_SDK_VERSION},
    },
};

/// Create a new D3D11 device.
pub fn create_d3d11_device() -> Result<ID3D11Device> {
    let feature_levels = [
        Direct3D::D3D_FEATURE_LEVEL_12_1,
        Direct3D::D3D_FEATURE_LEVEL_12_0,
        Direct3D::D3D_FEATURE_LEVEL_11_1,
        Direct3D::D3D_FEATURE_LEVEL_11_0,
        Direct3D::D3D_FEATURE_LEVEL_10_1,
        Direct3D::D3D_FEATURE_LEVEL_10_0,
        Direct3D::D3D_FEATURE_LEVEL_9_1,
    ];

    #[cfg(debug_assertions)]
    let flags = Direct3D11::D3D11_CREATE_DEVICE_DEBUG;

    #[cfg(not(debug_assertions))]
    let flags = Direct3D11::D3D11_CREATE_DEVICE_FLAG(0);

    let mut device = None;

    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            flags,
            Some(feature_levels.as_slice()),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )?;
    }

    let device = device.unwrap();
    let device_context = unsafe {
        let mut tmp = None;
        device.GetImmediateContext(&mut tmp);
        tmp.unwrap()
    };

    let multithreaded: ID3D11Multithread = device_context.cast().unwrap();
    unsafe {
        // Needed to prevent random deadlocks. The performance cost is quite negligible.
        multithreaded.SetMultithreadProtected(true);
    }

    Ok(device)
}

#[test]
fn test_d3d11_device_creation() {
    create_d3d11_device().unwrap();
}

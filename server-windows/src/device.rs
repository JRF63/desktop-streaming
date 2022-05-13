use windows::{
    core::Result,
    Win32::Graphics::{
        Direct3D::{self, D3D_DRIVER_TYPE_HARDWARE},
        Direct3D11::{
            D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_DEBUG, D3D11_SDK_VERSION,
            D3D11_CREATE_DEVICE_FLAG
        },
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

    let mut flags = D3D11_CREATE_DEVICE_FLAG(0);
    
    // TODO: D3D11_CREATE_DEVICE_SINGLETHREADED for release?
    #[cfg(debug_assertions)]
    {
        flags |= D3D11_CREATE_DEVICE_DEBUG;
    }

    let mut device = None;

    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            flags,
            &feature_levels,
            D3D11_SDK_VERSION,
            &mut device,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )?;
    }

    Ok(device.unwrap())
}

#[test]
fn test_d3d11_device_creation() {
    create_d3d11_device().unwrap();
}
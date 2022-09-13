use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM,
    DXGI_FORMAT_R8G8B8A8_UNORM,
};

impl crate::util::IntoNvEncBufferFormat for DXGI_FORMAT {
    fn into_nvenc_buffer_format(&self) -> crate::sys::NV_ENC_BUFFER_FORMAT {
        match *self {
            DXGI_FORMAT_B8G8R8A8_UNORM => {
                crate::sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_ARGB
            }
            DXGI_FORMAT_R10G10B10A2_UNORM => {
                crate::sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_ABGR10
            }
            DXGI_FORMAT_R8G8B8A8_UNORM => {
                crate::sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_ABGR
            }
            _ => crate::sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_UNDEFINED,
        }
    }
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use ::windows::Win32::Graphics::Direct3D11::ID3D11Texture2D as Texture;

use crate::Result;
use std::ffi::c_void;

pub trait IntoNvEncBufferFormat {
    fn into_nvenc_buffer_format(&self) -> crate::sys::NV_ENC_BUFFER_FORMAT;
}

pub trait TextureImplTrait {
    type TextureFormat: IntoNvEncBufferFormat;

    fn resource_type() -> crate::sys::NV_ENC_INPUT_RESOURCE_TYPE;

    fn description(&self) -> (u32, u32, Self::TextureFormat);

    fn as_ptr(&self) -> *mut c_void;

    fn build_register_resource_args(
        &self,
        pitch: Option<u32>,
        subresource_index: Option<u32>,
    ) -> Result<crate::sys::NV_ENC_REGISTER_RESOURCE>;
}
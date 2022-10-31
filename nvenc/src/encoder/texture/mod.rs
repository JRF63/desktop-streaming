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
    fn resource_type() -> crate::sys::NV_ENC_INPUT_RESOURCE_TYPE;

    fn as_ptr(&self) -> *mut c_void;

    fn build_register_resource_args(
        &self,
        pitch_or_subresource_index: u32,
    ) -> Result<crate::sys::NV_ENC_REGISTER_RESOURCE>;
}

pub trait TextureBufferImplTrait {
    type Texture: TextureImplTrait;
    type TextureFormat: IntoNvEncBufferFormat;

    fn texture_format(&self) -> Self::TextureFormat;

    fn get_texture(&self, index: usize) -> &Self::Texture;

    fn get_pitch_or_subresource_index(&self, index: usize) -> u32;
}

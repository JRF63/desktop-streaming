#[cfg(windows)]
mod windows;

use crate::Result;
use std::ffi::c_void;
use super::texture::TextureImplTrait;

pub trait DeviceImplTrait {
    type Texture: TextureImplTrait;

    fn device_type() -> crate::sys::NV_ENC_DEVICE_TYPE;

    fn create_texture_buffer(
        width: u32,
        height: u32,
        texture_format: <Self::Texture as TextureImplTrait>::TextureFormat,
        buf_size: u32,
    ) -> Result<Self::Texture>;

    fn as_ptr(&self) -> *mut c_void;
}


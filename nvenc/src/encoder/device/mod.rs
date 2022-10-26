#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use self::windows::DirectXDevice as Device;

use super::texture::TextureImplTrait;
use crate::Result;
use std::ffi::c_void;

/// Methods needed to be implemented by a NvEnc device.
pub trait DeviceImplTrait {
    /// Native texture used by the device.
    type Texture: TextureImplTrait;

    /// The device type required by `NvEncOpenEncodeSessionEx`.
    fn device_type() -> crate::sys::NV_ENC_DEVICE_TYPE;

    /// Pointer to the device need when initializing an encode session.
    fn as_ptr(&self) -> *mut c_void;

    /// Creates a texture buffer where input frames can be staged. This is desirable so that
    /// the NvEnc API does not need to coordinate when to release/unmap the input resource with the
    /// caller.
    fn create_texture_buffer(
        &self,
        width: u32,
        height: u32,
        texture_format: <Self::Texture as TextureImplTrait>::TextureFormat,
        buf_size: u32,
    ) -> Result<Self::Texture>;

    /// Copy a texture to the given buffer.
    fn copy_texture(
        &self,
        buffer: &Self::Texture,
        texture: &Self::Texture,
        subresource_index: usize,
    );
}

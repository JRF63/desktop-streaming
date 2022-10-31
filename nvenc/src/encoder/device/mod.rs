#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use self::windows::DirectX11Device as Device;

use super::texture::{TextureImplTrait, TextureBufferImplTrait};
use crate::Result;
use std::ffi::c_void;

/// Methods needed to be implemented by a NvEnc device.
pub trait DeviceImplTrait {
    /// Texture buffer for staging input frames.
    type Buffer: TextureBufferImplTrait;
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
        texture_format: <Self::Buffer as TextureBufferImplTrait>::TextureFormat,
        buf_size: u32,
    ) -> Result<Self::Buffer>;

    /// Copy a texture to the given buffer.
    fn copy_texture(
        &self,
        buffer: &Self::Buffer,
        texture: Self::Texture,
        subresource_index: usize,
    );
}

pub trait IntoDevice {
    type Device: DeviceImplTrait;

    fn into_device(self) -> Self::Device;
}
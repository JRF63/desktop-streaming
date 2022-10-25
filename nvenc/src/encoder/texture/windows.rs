use super::{IntoNvEncBufferFormat, TextureImplTrait};
use crate::{NvEncError, Result};
use std::mem::MaybeUninit;
use windows::Win32::Graphics::{
    Direct3D11::ID3D11Texture2D,
    Dxgi::Common::{
        DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM,
        DXGI_FORMAT_R8G8B8A8_UNORM,
    },
};

impl TextureImplTrait for ID3D11Texture2D {
    type TextureFormat = DXGI_FORMAT;

    fn resource_type() -> crate::sys::NV_ENC_INPUT_RESOURCE_TYPE {
        crate::sys::NV_ENC_INPUT_RESOURCE_TYPE::NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX
    }

    fn as_ptr(&self) -> *mut std::os::raw::c_void {
        unsafe { std::mem::transmute(self.clone()) }
    }

    fn description(&self) -> (u32, u32, Self::TextureFormat) {
        let texture_desc = unsafe {
            let mut tmp = MaybeUninit::uninit();
            self.GetDesc(tmp.as_mut_ptr());
            tmp.assume_init()
        };
        (texture_desc.Width, texture_desc.Height, texture_desc.Format)
    }

    fn build_register_resource_args(
        &self,
        pitch: Option<u32>,
        subresource_index: Option<u32>,
    ) -> Result<crate::sys::NV_ENC_REGISTER_RESOURCE> {
        debug_assert!(pitch.is_none(), "DirectX resources should not set pitch");

        let (width, height, format) = self.description();
        let subresource_index =
            subresource_index.ok_or(NvEncError::RegisterResourceMissingSubresourceIndex)?;

        let register_resource_args = crate::sys::NV_ENC_REGISTER_RESOURCE {
            version: crate::sys::NV_ENC_REGISTER_RESOURCE_VER,
            resourceType: Self::resource_type(),
            width,
            height,
            pitch: 0,
            subResourceIndex: subresource_index,
            resourceToRegister: self.as_ptr(),
            registeredResource: std::ptr::null_mut(),
            bufferFormat: format.into_nvenc_buffer_format(),
            bufferUsage: crate::sys::NV_ENC_BUFFER_USAGE::NV_ENC_INPUT_IMAGE,
            pInputFencePoint: std::ptr::null_mut(),
            pOutputFencePoint: std::ptr::null_mut(),
            reserved1: [0; 247],
            reserved2: [std::ptr::null_mut(); 60],
        };
        Ok(register_resource_args)
    }
}

impl IntoNvEncBufferFormat for DXGI_FORMAT {
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

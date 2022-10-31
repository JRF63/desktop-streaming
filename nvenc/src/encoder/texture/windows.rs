use super::{IntoNvEncBufferFormat, TextureBufferImplTrait, TextureImplTrait};
use crate::Result;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use windows::{
    core::Interface,
    Win32::Graphics::{
        Direct3D11::ID3D11Texture2D,
        Dxgi::Common::{
            DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM,
            DXGI_FORMAT_R8G8B8A8_UNORM,
        },
    },
};

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

impl TextureImplTrait for ID3D11Texture2D {
    fn resource_type() -> crate::sys::NV_ENC_INPUT_RESOURCE_TYPE {
        crate::sys::NV_ENC_INPUT_RESOURCE_TYPE::NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX
    }

    fn as_ptr(&self) -> *mut c_void {
        self.as_raw()
    }

    fn build_register_resource_args(
        &self,
        pitch_or_subresource_index: u32,
    ) -> Result<crate::sys::NV_ENC_REGISTER_RESOURCE> {
        let (width, height, format) = {
            let texture_desc = unsafe {
                let mut tmp = MaybeUninit::uninit();
                self.GetDesc(tmp.as_mut_ptr());
                tmp.assume_init()
            };
            (texture_desc.Width, texture_desc.Height, texture_desc.Format)
        };
        let subresource_index = pitch_or_subresource_index;

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

impl TextureBufferImplTrait for ID3D11Texture2D {
    type Texture = ID3D11Texture2D;
    type TextureFormat = DXGI_FORMAT;

    fn texture_format(&self) -> Self::TextureFormat {
        let texture_desc = unsafe {
            let mut tmp = MaybeUninit::uninit();
            self.GetDesc(tmp.as_mut_ptr());
            tmp.assume_init()
        };
        texture_desc.Format
    }

    fn get_texture(&self, _index: usize) -> &Self::Texture {
        self
    }

    fn get_pitch_or_subresource_index(&self, index: usize) -> u32 {
        index as u32
    }
}

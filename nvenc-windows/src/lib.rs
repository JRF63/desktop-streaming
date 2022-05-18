mod error;
mod guids;
mod init;
mod output;
mod queries;
mod util;

use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE},
    Graphics::{
        Direct3D11::{
            ID3D11Device, ID3D11Device1, ID3D11DeviceContext, ID3D11Query, ID3D11Resource,
            ID3D11Texture2D, D3D11_BIND_FLAG, D3D11_CPU_ACCESS_FLAG, D3D11_QUERY_DESC,
            D3D11_QUERY_EVENT, D3D11_RESOURCE_MISC_SHARED, D3D11_RESOURCE_MISC_SHARED_NTHANDLE,
            D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
        },
        Dxgi::{
            Common::DXGI_SAMPLE_DESC, IDXGIResource, IDXGIResource1, DXGI_OUTDUPL_DESC,
            DXGI_SHARED_RESOURCE_READ,
        },
    },
    System::Threading::CreateEventA,
};

#[macro_export]
macro_rules! nvenc_function {
    ($fn:expr, $($arg:expr),*) => {
        let status = ($fn.unwrap_or_else(|| std::hint::unreachable_unchecked()))($($arg,)*);
        if status != nvenc_sys::NVENCSTATUS::NV_ENC_SUCCESS {
            return Err(crate::error::NvEncError::new(status));
        }
    }
}

pub type Result<T> = std::result::Result<T, error::NvEncError>;

pub(crate) struct EncoderIO {
    d3d11_texture: ID3D11Texture2D,
    registered_resource: NonNull<c_void>,
    input_ptr: nvenc_sys::NV_ENC_INPUT_PTR,
    output_ptr: NonNull<c_void>,
    event_obj: HANDLE,
}

unsafe impl Sync for EncoderIO {}

// TODO: Pull out the function list into a global struct?
pub(crate) struct RawEncoder<const BUF_SIZE: usize> {
    raw_encoder: NonNull<c_void>,
    functions: nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    io: [EncoderIO; BUF_SIZE],
    library: HINSTANCE,
}

impl<const BUF_SIZE: usize> Drop for RawEncoder<BUF_SIZE> {
    fn drop(&mut self) {
        // TODO: Prob should log the errors instead of ignoring them.
        for io in &self.io {
            unsafe {
                (self.functions.nvEncUnmapInputResource.unwrap())(
                    self.raw_encoder.as_ptr(),
                    io.input_ptr,
                );
                (self.functions.nvEncUnregisterResource.unwrap())(
                    self.raw_encoder.as_ptr(),
                    io.registered_resource.as_ptr(),
                );
                (self.functions.nvEncDestroyBitstreamBuffer.unwrap())(
                    self.raw_encoder.as_ptr(),
                    io.output_ptr.as_ptr(),
                );
                CloseHandle(io.event_obj);
            }
        }
        unsafe {
            (self.functions.nvEncDestroyEncoder.unwrap())(self.raw_encoder.as_ptr());
        }
        crate::init::free_library(self.library);
    }
}

unsafe impl<const BUF_SIZE: usize> Sync for RawEncoder<BUF_SIZE> {}

impl<const BUF_SIZE: usize> RawEncoder<BUF_SIZE> {
    /// Registers the passed texture for NVENC API bookkeeping.
    fn register_input_buffer(&self, d3d11_texture: ID3D11Texture2D) -> Result<NonNull<c_void>> {
        let mut texture_desc = MaybeUninit::uninit();
        let texture_desc = unsafe {
            d3d11_texture.GetDesc(texture_desc.as_mut_ptr());
            texture_desc.assume_init()
        };

        let mut register_resource_params = nvenc_sys::NV_ENC_REGISTER_RESOURCE {
            version: nvenc_sys::NV_ENC_REGISTER_RESOURCE_VER,
            resourceType: nvenc_sys::NV_ENC_INPUT_RESOURCE_TYPE::NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX,
            width: texture_desc.Width,
            height: texture_desc.Height,
            pitch: 0,
            subResourceIndex: 0,
            resourceToRegister: unsafe { std::mem::transmute(d3d11_texture) }, // cast to *mut c_void,
            registeredResource: std::ptr::null_mut(),
            bufferFormat: util::dxgi_to_nv_format(texture_desc.Format),
            bufferUsage: nvenc_sys::NV_ENC_BUFFER_USAGE::NV_ENC_INPUT_IMAGE,
            pInputFencePoint: std::ptr::null_mut(),
            pOutputFencePoint: std::ptr::null_mut(),
            reserved1: [0; 247],
            reserved2: [std::ptr::null_mut(); 60],
        };

        unsafe {
            nvenc_function!(
                self.functions.nvEncRegisterResource,
                self.raw_encoder.as_ptr(),
                &mut register_resource_params
            );
        }

        Ok(NonNull::new(register_resource_params.registeredResource).unwrap())
    }

    /// Allocate an output buffer. Should be called only after the encoder has been configured.
    fn create_output_buffers(&mut self) -> Result<NonNull<c_void>> {
        let mut create_bitstream_buffer_params: nvenc_sys::NV_ENC_CREATE_BITSTREAM_BUFFER =
            unsafe { std::mem::zeroed() };
        create_bitstream_buffer_params.version = nvenc_sys::NV_ENC_CREATE_BITSTREAM_BUFFER_VER;

        unsafe {
            nvenc_function!(
                self.functions.nvEncCreateBitstreamBuffer,
                self.raw_encoder.as_ptr(),
                &mut create_bitstream_buffer_params
            );

            // *output_buffer = create_bitstream_buffer_params.bitstreamBuffer;
        }
        Ok(NonNull::new(create_bitstream_buffer_params.bitstreamBuffer).unwrap())
    }

    /// Creates an `ID3D11Texture2D` where the duplicated frame can be copied to.
    fn create_input_buffer(
        d3d11_device: &ID3D11Device,
        display_desc: &DXGI_OUTDUPL_DESC,
    ) -> windows::core::Result<ID3D11Texture2D> {
        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: display_desc.ModeDesc.Width,
            Height: display_desc.ModeDesc.Height,
            // plain display output has only one mip
            MipLevels: 1,
            ArraySize: 1,
            Format: display_desc.ModeDesc.Format,
            SampleDesc: DXGI_SAMPLE_DESC {
                // default sampler mode
                Count: 1,
                // default sampler mode
                Quality: 0,
            },
            // GPU needs read/write access
            Usage: D3D11_USAGE_DEFAULT,
            // TODO: what flag to use?
            BindFlags: D3D11_BIND_FLAG(0),
            // don't need to be accessed by the CPU
            CPUAccessFlags: D3D11_CPU_ACCESS_FLAG(0),
            // shared with the encoder that has a "different" GPU handle,
            // NTHANDLE to be able to use `CreateSharedHandle` and pass
            // DXGI_SHARED_RESOURCE_READ
            MiscFlags: D3D11_RESOURCE_MISC_SHARED | D3D11_RESOURCE_MISC_SHARED_NTHANDLE,
        };

        unsafe {
            let input_buffer = d3d11_device.CreateTexture2D(&texture_desc, std::ptr::null())?;
            Ok(input_buffer)
        }
    }

    fn create_event_object() -> windows::core::Result<HANDLE> {
        unsafe { CreateEventA(std::ptr::null(), false, false, None) }
    }
}

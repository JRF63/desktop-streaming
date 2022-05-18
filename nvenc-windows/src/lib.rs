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
        Direct3D11::ID3D11Texture2D,
        
    },
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
    texture: ID3D11Texture2D,
    registered_resource: NonNull<c_void>,
    input_ptr: nvenc_sys::NV_ENC_INPUT_PTR,
    output_ptr: NonNull<c_void>,
    event_obj: HANDLE,
}

unsafe impl Sync for EncoderIO {}

// TODO: Pull out the function list into a global struct?
pub(crate) struct NvidiaEncoder<const BUF_SIZE: usize> {
    raw_encoder: NonNull<c_void>,
    functions: nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    io: [EncoderIO; BUF_SIZE],
    library: HINSTANCE,
}

impl<const BUF_SIZE: usize> Drop for NvidiaEncoder<BUF_SIZE> {
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
                crate::init::destroy_event_object(io.event_obj);
            }
        }
        unsafe {
            (self.functions.nvEncDestroyEncoder.unwrap())(self.raw_encoder.as_ptr());
        }
        crate::init::free_library(self.library);
    }
}

unsafe impl<const BUF_SIZE: usize> Sync for NvidiaEncoder<BUF_SIZE> {}

impl<const BUF_SIZE: usize> NvidiaEncoder<BUF_SIZE> {
    
}

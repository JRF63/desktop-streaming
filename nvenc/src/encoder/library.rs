#[cfg(windows)]
use crate::os::windows::WindowsLibrary as LibraryImpl;

use crate::{NvEncError, Result};
use std::mem::MaybeUninit;

pub(crate) struct NvidiaEncoderLibrary(LibraryImpl);

impl NvidiaEncoderLibrary {
    pub(crate) fn load() -> Result<Self> {
        #[cfg(windows)]
        const LIBRARY_NAME: &'static str = "nvEncodeAPI64.dll";

        if !LibraryImpl::is_library_signed(LIBRARY_NAME) {
            Err(NvEncError::LibraryNotSigned)
        } else {
            match LibraryImpl::load(LIBRARY_NAME) {
                Ok(lib) => Ok(NvidiaEncoderLibrary(lib)),
                Err(_) => Err(NvEncError::LibraryLoadingFailed),
            }
        }
    }

    fn as_inner(&self) -> &LibraryImpl {
        &self.0
    }

    pub(crate) fn get_max_supported_version(&self) -> Result<u32> {
        const FN_NAME: &'static str = "NvEncodeAPIGetMaxSupportedVersion";
        type GetMaxSupportedVersion = unsafe extern "C" fn(*mut u32) -> crate::sys::NVENCSTATUS;

        let get_max_supported_version: GetMaxSupportedVersion = unsafe {
            let tmp = self
                .as_inner()
                .fn_ptr(FN_NAME)
                .or(Err(NvEncError::GetMaxSupportedVersionLoadingFailed))?;
            std::mem::transmute(tmp)
        };

        let mut version: u32 = 0;
        let status = unsafe { get_max_supported_version(&mut version) };
        match NvEncError::from_nvenc_status(status) {
            Some(err) => Err(err),
            None => Ok(version),
        }
    }

    pub(crate) fn create_instance(&self) -> Result<crate::sys::NV_ENCODE_API_FUNCTION_LIST> {
        const FN_NAME: &'static str = "NvEncodeAPICreateInstance";
        type CreateInstance = unsafe extern "C" fn(
            *mut crate::sys::NV_ENCODE_API_FUNCTION_LIST,
        ) -> crate::sys::NVENCSTATUS;

        let create_instance: CreateInstance = unsafe {
            let tmp = self
                .as_inner()
                .fn_ptr(FN_NAME)
                .or(Err(NvEncError::CreateInstanceLoadingFailed))?;
            std::mem::transmute(tmp)
        };

        unsafe {
            let mut fn_list: crate::sys::NV_ENCODE_API_FUNCTION_LIST =
                MaybeUninit::zeroed().assume_init();
            // The version needs to be set or the API will return an error
            fn_list.version = crate::sys::NV_ENCODE_API_FUNCTION_LIST_VER;
            let status = create_instance(&mut fn_list);
            match NvEncError::from_nvenc_status(status) {
                Some(err) => Err(err),
                None => Ok(fn_list),
            }
        }
    }
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
use self::windows::LibraryImpl;

use crate::{NvEncError, Result};
use std::mem::MaybeUninit;

/// Helper trait that needs to be implemented by OS-specific library implementation. 
trait LibraryImplTrait: Sized {
    /// Filename of the .dll or .so
    const LIBRARY_NAME: &'static str;

    /// Checks if the library is signed.
    fn is_library_signed(filename: &str) -> bool;

    /// Load an instance of the library.
    fn load(lib_name: &str) -> Result<Self>;

    /// Extracts a function pointer from the library.
    unsafe fn fn_ptr<T>(&self, fn_name: &str) -> Option<T>;
}

pub struct Library(LibraryImpl);

impl Library {
    pub fn load() -> Result<Self> {
        if !LibraryImpl::is_library_signed(LibraryImpl::LIBRARY_NAME) {
            Err(NvEncError::LibraryNotSigned)
        } else {
            LibraryImpl::load(LibraryImpl::LIBRARY_NAME).map(|lib| Library(lib))
        }
    }

    fn as_inner(&self) -> &LibraryImpl {
        &self.0
    }

    pub fn get_max_supported_version(&self) -> Result<u32> {
        const FN_NAME: &'static str = "NvEncodeAPIGetMaxSupportedVersion";
        type GetMaxSupportedVersion = unsafe extern "C" fn(*mut u32) -> crate::sys::NVENCSTATUS;

        let get_max_supported_version: GetMaxSupportedVersion = unsafe {
            self.as_inner()
                .fn_ptr(FN_NAME)
                .ok_or(NvEncError::GetMaxSupportedVersionLoadingFailed)?
        };
        
        let mut version: u32 = 0;
        let status = unsafe { get_max_supported_version(&mut version) };

        match NvEncError::from_nvenc_status(status) {
            Some(err) => Err(err),
            None => Ok(version),
        }
    }

    pub fn get_function_list(&self) -> Result<crate::sys::NV_ENCODE_API_FUNCTION_LIST> {
        const FN_NAME: &'static str = "NvEncodeAPICreateInstance";
        type CreateInstance = unsafe extern "C" fn(
            *mut crate::sys::NV_ENCODE_API_FUNCTION_LIST,
        ) -> crate::sys::NVENCSTATUS;

        let create_instance: CreateInstance = unsafe {
            self.as_inner()
                .fn_ptr(FN_NAME)
                .ok_or(NvEncError::CreateInstanceLoadingFailed)?
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

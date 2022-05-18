use std::{ffi::CString, mem::MaybeUninit, os::raw::c_void, ptr::NonNull};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::HINSTANCE,
        System::LibraryLoader::{
            FreeLibrary, GetProcAddress, LoadLibraryExA, LOAD_LIBRARY_REQUIRE_SIGNED_TARGET,
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        },
    },
};
use crate::RawEncoder;

pub(crate) fn load_library(lib_name: &str) -> Option<HINSTANCE> {
    let lib_name = CString::new(lib_name).unwrap();
    let load_result = unsafe {
        LoadLibraryExA(
            PCSTR(lib_name.as_ptr() as *const u8),
            None,
            LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_REQUIRE_SIGNED_TARGET,
        )
    };
    load_result.ok()
}

pub(crate) fn free_library(lib: HINSTANCE) {
    unsafe {
        // Deliberately ignoring failure
        FreeLibrary(lib);
    }
}

pub(crate) fn load_nvenc_library() -> Option<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST> {
    let lib_name = CString::new("nvEncodeAPI64.dll").unwrap();
    let load_result = unsafe {
        LoadLibraryExA(
            PCSTR(lib_name.as_ptr() as *const u8),
            None,
            LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_REQUIRE_SIGNED_TARGET,
        )
    };
    match load_result {
        Ok(lib) => {
            if lib.0 != 0 {
                if is_version_supported(lib)? {
                    return get_function_list(lib);
                }
            }
            None
        }
        Err(_) => None,
    }
}

/// Extracts the function pointer from the library.
fn fn_from_lib(lib: HINSTANCE, fn_name: &str) -> Option<unsafe extern "system" fn() -> isize> {
    let fn_name = CString::new(fn_name).unwrap();
    let fn_ptr = unsafe { GetProcAddress(lib, PCSTR(fn_name.as_ptr() as *const u8)) };
    fn_ptr
}

/// Checks if the user's NvEncAPI version is supported.
fn is_version_supported(lib: HINSTANCE) -> Option<bool> {
    let mut max_supported_version: u32 = 0;
    unsafe {
        let get_max_supported_version: unsafe extern "C" fn(*mut u32) -> nvenc_sys::NVENCSTATUS =
            std::mem::transmute(fn_from_lib(lib, "NvEncodeAPIGetMaxSupportedVersion")?);
        let status = get_max_supported_version(&mut max_supported_version);
        if status != nvenc_sys::NVENCSTATUS::NV_ENC_SUCCESS {
            return None;
        }
    }
    if max_supported_version >= nvenc_sys::NVENCAPI_VERSION {
        Some(true)
    } else {
        Some(false)
    }
}

/// Load the struct containing the NvEncAPI function pointers.
fn get_function_list(lib: HINSTANCE) -> Option<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST> {
    // Need to zero the struct before passing to `NvEncodeAPICreateInstance`
    let mut fn_list = MaybeUninit::<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST>::zeroed();
    let fn_list = unsafe {
        // Set the version of the function list struct
        (&mut (*fn_list.as_mut_ptr())).version = nvenc_sys::NV_ENCODE_API_FUNCTION_LIST_VER;

        let create_instance: unsafe extern "C" fn(
            *mut nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        ) -> nvenc_sys::NVENCSTATUS =
            std::mem::transmute(fn_from_lib(lib, "NvEncodeAPICreateInstance")?);
        if create_instance(fn_list.as_mut_ptr()) != nvenc_sys::NVENCSTATUS::NV_ENC_SUCCESS {
            return None;
        }
        fn_list.assume_init()
    };

    // The function list was initialized with zero, so this should not be a null pointer when
    // the call to `NvEncodeAPICreateInstance` succeeded
    if fn_list.nvEncOpenEncodeSession.is_some() {
        Some(fn_list)
    } else {
        None
    }
}

/// Start an encoding session.
pub(crate) fn open_encode_session(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    device: NonNull<c_void>,
    device_type: nvenc_sys::NV_ENC_DEVICE_TYPE,
) -> Option<NonNull<c_void>> {
    let mut session_params: nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS =
        unsafe { std::mem::zeroed() };
    session_params.version = nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
    session_params.deviceType = device_type;
    session_params.device = device.as_ptr();
    session_params.apiVersion = nvenc_sys::NVENCAPI_VERSION;

    let mut encoder: *mut c_void = std::ptr::null_mut();
    unsafe {
        let status = (functions
            .nvEncOpenEncodeSessionEx
            .unwrap_or_else(|| std::hint::unreachable_unchecked()))(
            &mut session_params,
            &mut encoder,
        );
        if status == nvenc_sys::NVENCSTATUS::NV_ENC_SUCCESS {
            Some(NonNull::new_unchecked(encoder))
        } else {
            None
        }
    }
}

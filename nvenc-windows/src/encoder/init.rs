use crate::{error::NvEncError, nvenc_function, util::NvEncDevice, Result};
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull};

// TODO: Make this a generic parameter
use crate::os::windows::WindowsLibrary;

/// Checks if the user's NvEncAPI version is supported.
pub(crate) fn is_version_supported(lib: &WindowsLibrary) -> Result<bool> {
    let mut version: u32 = 0;
    unsafe {
        let fn_ptr = lib
            .fn_ptr("NvEncodeAPIGetMaxSupportedVersion")
            .or(Err(NvEncError::GetMaxSupportedVersionLoadingFailed))?;

        type GetMaxSupportedVersion = unsafe extern "C" fn(*mut u32) -> nvenc_sys::NVENCSTATUS;
        let get_max_supported_version: GetMaxSupportedVersion = std::mem::transmute(fn_ptr);

        let status = get_max_supported_version(&mut version);
        if let Some(error) = NvEncError::from_nvenc_status(status) {
            return Err(error);
        }
    }
    let major_version = version >> 4;
    let minor_version = version & 0b1111;
    if major_version >= nvenc_sys::NVENCAPI_MAJOR_VERSION
        && minor_version >= nvenc_sys::NVENCAPI_MINOR_VERSION
    {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Load the struct containing the NvEncAPI function pointers.
pub(crate) fn get_function_list(lib: &WindowsLibrary) -> Result<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST> {
    // Zeroes the version and setss all the function pointers to `None`
    let mut fn_list = MaybeUninit::<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST>::zeroed();

    let fn_list = unsafe {
        // Set the version of the function list struct
        (&mut (*fn_list.as_mut_ptr())).version = nvenc_sys::NV_ENCODE_API_FUNCTION_LIST_VER;

        let fn_ptr = lib
            .fn_ptr("NvEncodeAPICreateInstance")
            .or(Err(NvEncError::CreateInstanceLoadingFailed))?;

        type CreateInstance = unsafe extern "C" fn(
            *mut nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        ) -> nvenc_sys::NVENCSTATUS;
        let create_instance: CreateInstance = std::mem::transmute(fn_ptr);

        let status = create_instance(fn_list.as_mut_ptr());
        if let Some(error) = NvEncError::from_nvenc_status(status) {
            return Err(error);
        }
        fn_list.assume_init()
    };

    // Test all the pointers since the `nvenc_function!` macro is doing `unwrap_unchecked`
    let test_function_pointers_for_nulls = || -> Option<()> {
        fn_list.nvEncOpenEncodeSession?;
        fn_list.nvEncGetEncodeGUIDCount?;
        fn_list.nvEncGetEncodeProfileGUIDCount?;
        fn_list.nvEncGetEncodeProfileGUIDs?;
        fn_list.nvEncGetEncodeGUIDs?;
        fn_list.nvEncGetInputFormatCount?;
        fn_list.nvEncGetInputFormats?;
        fn_list.nvEncGetEncodeCaps?;
        fn_list.nvEncGetEncodePresetCount?;
        fn_list.nvEncGetEncodePresetGUIDs?;
        fn_list.nvEncGetEncodePresetConfig?;
        fn_list.nvEncInitializeEncoder?;
        fn_list.nvEncCreateInputBuffer?;
        fn_list.nvEncDestroyInputBuffer?;
        fn_list.nvEncCreateBitstreamBuffer?;
        fn_list.nvEncDestroyBitstreamBuffer?;
        fn_list.nvEncEncodePicture?;
        fn_list.nvEncLockBitstream?;
        fn_list.nvEncUnlockBitstream?;
        fn_list.nvEncLockInputBuffer?;
        fn_list.nvEncUnlockInputBuffer?;
        fn_list.nvEncGetEncodeStats?;
        fn_list.nvEncGetSequenceParams?;
        fn_list.nvEncRegisterAsyncEvent?;
        fn_list.nvEncUnregisterAsyncEvent?;
        fn_list.nvEncMapInputResource?;
        fn_list.nvEncUnmapInputResource?;
        fn_list.nvEncDestroyEncoder?;
        fn_list.nvEncInvalidateRefFrames?;
        fn_list.nvEncOpenEncodeSessionEx?;
        fn_list.nvEncRegisterResource?;
        fn_list.nvEncUnregisterResource?;
        fn_list.nvEncReconfigureEncoder?;
        fn_list.nvEncCreateMVBuffer?;
        fn_list.nvEncDestroyMVBuffer?;
        fn_list.nvEncRunMotionEstimationOnly?;
        fn_list.nvEncGetLastErrorString?;
        fn_list.nvEncSetIOCudaStreams?;
        fn_list.nvEncGetEncodePresetConfigEx?;
        fn_list.nvEncGetSequenceParamEx?;
        Some(())
    };

    match test_function_pointers_for_nulls() {
        Some(_) => Ok(fn_list),
        None => Err(NvEncError::MalformedFunctionList),
    }
}

/// Start an encoding session.
pub(crate) fn open_encode_session<T: NvEncDevice>(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    device: &T,
) -> Result<NonNull<c_void>> {
    let mut raw_encoder: *mut c_void = std::ptr::null_mut();
    unsafe {
        let mut session_params: nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS =
            MaybeUninit::zeroed().assume_init();
        session_params.version = nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
        session_params.deviceType = T::device_type();
        session_params.device = device.as_ptr();
        session_params.apiVersion = nvenc_sys::NVENCAPI_VERSION;

        nvenc_function!(
            functions.nvEncOpenEncodeSessionEx,
            &mut session_params,
            &mut raw_encoder
        );
    }

    match NonNull::new(raw_encoder) {
        Some(ptr) => Ok(ptr),
        None => Err(NvEncError::Generic),
    }
}

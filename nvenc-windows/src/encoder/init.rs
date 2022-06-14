use crate::{error::NvEncError, nvenc_function, util::IntoNvEncBufferFormat, Result};
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};

use crate::os::windows::{EventObject, Library};

/// Checks if the user's NvEncAPI version is supported.
pub(crate) fn is_version_supported(lib: &Library) -> anyhow::Result<bool> {
    let mut version: u32 = 0;
    unsafe {
        let get_max_supported_version: unsafe extern "C" fn(*mut u32) -> nvenc_sys::NVENCSTATUS =
            std::mem::transmute(lib.fn_ptr("NvEncodeAPIGetMaxSupportedVersion")?);
        let status = get_max_supported_version(&mut version);
        if let Some(error) = NvEncError::from_nvenc_status(status) {
            return Err(error.into());
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
pub(crate) fn get_function_list(lib: &Library) -> Result<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST> {
    // Zeroes the version and setss all the function pointers to `None`
    let mut fn_list = MaybeUninit::<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST>::zeroed();

    let fn_list = unsafe {
        // Set the version of the function list struct
        (&mut (*fn_list.as_mut_ptr())).version = nvenc_sys::NV_ENCODE_API_FUNCTION_LIST_VER;

        if let Ok(fn_ptr) = lib.fn_ptr("NvEncodeAPICreateInstance") {
            let create_instance: unsafe extern "C" fn(
                *mut nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
            ) -> nvenc_sys::NVENCSTATUS = std::mem::transmute(fn_ptr);

            let status = create_instance(fn_list.as_mut_ptr());
            if let Some(error) = NvEncError::from_nvenc_status(status) {
                return Err(error.into());
            }
            fn_list.assume_init()
        } else {
            return Err(NvEncError::FunctionListLoadingFailed);
        }
    };

    // The function list was initialized with zero, so this should not be a null pointer when
    // the call to `NvEncodeAPICreateInstance` succeeded
    if fn_list.nvEncOpenEncodeSession.is_some() {
        Ok(fn_list)
    } else {
        Err(NvEncError::MalformedFunctionList)
    }
}

/// Start an encoding session.
pub(crate) fn open_encode_session(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    device: ID3D11Device,
) -> Result<NonNull<c_void>> {
    let mut session_params: nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS =
        unsafe { MaybeUninit::zeroed().assume_init() };
    session_params.version = nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
    session_params.deviceType = nvenc_sys::NV_ENC_DEVICE_TYPE::NV_ENC_DEVICE_TYPE_DIRECTX;
    session_params.device = unsafe { std::mem::transmute(device) };
    session_params.apiVersion = nvenc_sys::NVENCAPI_VERSION;

    let mut raw_encoder: *mut c_void = std::ptr::null_mut();
    let status = unsafe {
        (functions.nvEncOpenEncodeSessionEx.unwrap_unchecked())(
            &mut session_params,
            &mut raw_encoder,
        )
    };

    match NvEncError::from_nvenc_status(status) {
        Some(error) => Err(error.into()),
        None => match NonNull::new(raw_encoder) {
            Some(ptr) => Ok(ptr),
            None => Err(NvEncError::OpenEncodeSessionFailed),
        },
    }
}

pub(crate) fn register_async_event(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
    event: &EventObject,
) -> Result<()> {
    unsafe {
        let mut event_params: nvenc_sys::NV_ENC_EVENT_PARAMS = MaybeUninit::zeroed().assume_init();
        event_params.version = nvenc_sys::NV_ENC_EVENT_PARAMS_VER;
        event_params.completionEvent = event.as_ptr();
        nvenc_function!(
            functions.nvEncRegisterAsyncEvent,
            raw_encoder.as_ptr(),
            &mut event_params
        );
    }
    Ok(())
}

pub(crate) fn unregister_async_event(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
    event: &EventObject,
) -> Result<()> {
    unsafe {
        let mut event_params: nvenc_sys::NV_ENC_EVENT_PARAMS = MaybeUninit::zeroed().assume_init();
        event_params.version = nvenc_sys::NV_ENC_EVENT_PARAMS_VER;
        event_params.completionEvent = event.as_ptr();
        nvenc_function!(
            functions.nvEncUnregisterAsyncEvent,
            raw_encoder.as_ptr(),
            &mut event_params
        );
    }
    Ok(())
}

/// Registers the passed texture for NVENC API bookkeeping.
pub(crate) fn register_resource(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
    texture: ID3D11Texture2D,
    subresource_index: u32,
) -> Result<NonNull<c_void>> {
    let mut texture_desc = MaybeUninit::uninit();
    let texture_desc = unsafe {
        texture.GetDesc(texture_desc.as_mut_ptr());
        texture_desc.assume_init()
    };

    let mut register_resource_params = nvenc_sys::NV_ENC_REGISTER_RESOURCE {
        version: nvenc_sys::NV_ENC_REGISTER_RESOURCE_VER,
        resourceType: nvenc_sys::NV_ENC_INPUT_RESOURCE_TYPE::NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX,
        width: texture_desc.Width,
        height: texture_desc.Height,
        pitch: 0,
        subResourceIndex: subresource_index,
        resourceToRegister: unsafe { std::mem::transmute(texture) }, // cast to *mut c_void,
        registeredResource: std::ptr::null_mut(),
        bufferFormat: texture_desc.Format.into_nvenc_buffer_format(),
        bufferUsage: nvenc_sys::NV_ENC_BUFFER_USAGE::NV_ENC_INPUT_IMAGE,
        pInputFencePoint: std::ptr::null_mut(),
        pOutputFencePoint: std::ptr::null_mut(),
        reserved1: [0; 247],
        reserved2: [std::ptr::null_mut(); 60],
    };

    unsafe {
        nvenc_function!(
            functions.nvEncRegisterResource,
            raw_encoder.as_ptr(),
            &mut register_resource_params
        );
    }

    Ok(NonNull::new(register_resource_params.registeredResource).unwrap())
}

/// Allocate an output buffer. Should be called only after the encoder has been configured.
pub(crate) fn create_output_buffers(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
) -> Result<NonNull<c_void>> {
    let mut create_bitstream_buffer_params: nvenc_sys::NV_ENC_CREATE_BITSTREAM_BUFFER =
        unsafe { MaybeUninit::zeroed().assume_init() };
    create_bitstream_buffer_params.version = nvenc_sys::NV_ENC_CREATE_BITSTREAM_BUFFER_VER;

    unsafe {
        nvenc_function!(
            functions.nvEncCreateBitstreamBuffer,
            raw_encoder.as_ptr(),
            &mut create_bitstream_buffer_params
        );
    }
    Ok(NonNull::new(create_bitstream_buffer_params.bitstreamBuffer).unwrap())
}

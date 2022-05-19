use super::Result;
use crate::nvenc_function;
use std::{ffi::CString, mem::MaybeUninit, os::raw::c_void, ptr::NonNull};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, HINSTANCE},
        Graphics::{
            Direct3D11::{
                ID3D11Device, ID3D11Texture2D, D3D11_BIND_FLAG, D3D11_CPU_ACCESS_FLAG,
                D3D11_RESOURCE_MISC_SHARED, D3D11_RESOURCE_MISC_SHARED_NTHANDLE,
                D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
            },
            Dxgi::{Common::DXGI_SAMPLE_DESC, DXGI_OUTDUPL_DESC},
        },
        System::LibraryLoader::{
            FreeLibrary, GetProcAddress, LoadLibraryExA, LOAD_LIBRARY_REQUIRE_SIGNED_TARGET,
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        },
        System::Threading::CreateEventA,
    },
};

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

/// Extracts the function pointer from the library.
pub(crate) fn fn_ptr_from_lib(
    lib: HINSTANCE,
    fn_name: &str,
) -> Option<unsafe extern "system" fn() -> isize> {
    let fn_name = CString::new(fn_name).unwrap();
    let fn_ptr = unsafe { GetProcAddress(lib, PCSTR(fn_name.as_ptr() as *const u8)) };
    fn_ptr
}

/// Checks if the user's NvEncAPI version is supported.
pub(crate) fn is_version_supported(lib: HINSTANCE) -> Option<bool> {
    let mut max_supported_version: u32 = 0;
    unsafe {
        let get_max_supported_version: unsafe extern "C" fn(*mut u32) -> nvenc_sys::NVENCSTATUS =
            std::mem::transmute(fn_ptr_from_lib(lib, "NvEncodeAPIGetMaxSupportedVersion")?);
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
pub(crate) fn get_function_list(lib: HINSTANCE) -> Option<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST> {
    // Need to zero the struct before passing to `NvEncodeAPICreateInstance`
    let mut fn_list = MaybeUninit::<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST>::zeroed();
    let fn_list = unsafe {
        // Set the version of the function list struct
        (&mut (*fn_list.as_mut_ptr())).version = nvenc_sys::NV_ENCODE_API_FUNCTION_LIST_VER;

        let create_instance: unsafe extern "C" fn(
            *mut nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        ) -> nvenc_sys::NVENCSTATUS =
            std::mem::transmute(fn_ptr_from_lib(lib, "NvEncodeAPICreateInstance")?);
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
    device: ID3D11Device,
) -> Option<NonNull<c_void>> {
    let mut session_params: nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS =
        unsafe { std::mem::zeroed() };
    session_params.version = nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
    session_params.deviceType = nvenc_sys::NV_ENC_DEVICE_TYPE::NV_ENC_DEVICE_TYPE_DIRECTX;
    session_params.device = unsafe { std::mem::transmute(device) };
    session_params.apiVersion = nvenc_sys::NVENCAPI_VERSION;

    let mut raw_encoder: *mut c_void = std::ptr::null_mut();
    unsafe {
        let status = (functions
            .nvEncOpenEncodeSessionEx
            .unwrap_or_else(|| std::hint::unreachable_unchecked()))(
            &mut session_params,
            &mut raw_encoder,
        );
        if status == nvenc_sys::NVENCSTATUS::NV_ENC_SUCCESS {
            Some(NonNull::new_unchecked(raw_encoder))
        } else {
            None
        }
    }
}

/// Registers the passed texture for NVENC API bookkeeping.
pub(crate) fn register_resource(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
    texture: ID3D11Texture2D,
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
        subResourceIndex: 0,
        resourceToRegister: unsafe { std::mem::transmute(texture) }, // cast to *mut c_void,
        registeredResource: std::ptr::null_mut(),
        bufferFormat: crate::util::dxgi_to_nv_format(texture_desc.Format),
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
        unsafe { std::mem::zeroed() };
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

/// Creates an `ID3D11Texture2D` where the duplicated frame can be copied to.
pub(crate) fn create_texture_buffer(
    device: &ID3D11Device,
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
        let input_buffer = device.CreateTexture2D(&texture_desc, std::ptr::null())?;
        Ok(input_buffer)
    }
}

/// Create a Windows Event Object for signaling encoding completion of a frame.
pub(crate) fn create_event_object() -> windows::core::Result<HANDLE> {
    unsafe { CreateEventA(std::ptr::null(), false, false, None) }
}

/// Destroy a created Event Object.
pub(crate) fn destroy_event_object(event_obj: HANDLE) {
    unsafe { CloseHandle(event_obj) };
}

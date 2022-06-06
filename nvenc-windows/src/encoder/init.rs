use super::Result;
use crate::{error::NvEncError, nvenc_function};
use std::{ffi::CString, mem::MaybeUninit, os::raw::c_void, ptr::NonNull};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, HINSTANCE},
        Graphics::{
            Direct3D11::{
                ID3D11Device, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_FLAG,
                D3D11_RESOURCE_MISC_FLAG, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
            },
            Dxgi::{Common::DXGI_SAMPLE_DESC, DXGI_OUTDUPL_DESC},
        },
        System::LibraryLoader::{
            FreeLibrary, GetProcAddress, LoadLibraryExA, LOAD_LIBRARY_SEARCH_SYSTEM32,
        },
        System::{
            Threading::{CreateEventA, WaitForSingleObject, WAIT_OBJECT_0},
            WindowsProgramming::INFINITE,
        },
    },
};

#[repr(transparent)]
pub(crate) struct Library(HINSTANCE);

impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            // Deliberately ignoring failure
            FreeLibrary(self.0);
        }
    }
}

impl Library {
    /// Open a .dll.
    pub(crate) fn load(lib_name: &str) -> anyhow::Result<Self> {
        if !crate::os::windows::is_system_library_signed(lib_name) {
            anyhow::bail!("Library is not signed");
        }
        let lib_name = CString::new(lib_name).unwrap();
        let lib = unsafe {
            LoadLibraryExA(
                PCSTR(lib_name.as_ptr() as *const u8),
                None,
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        }?;
        Ok(Library(lib))
    }

    /// Extracts the function pointer from the library.
    pub(crate) fn fn_ptr(
        &self,
        fn_name: &str,
    ) -> windows::core::Result<unsafe extern "system" fn() -> isize> {
        let fn_name = CString::new(fn_name).unwrap();
        match unsafe { GetProcAddress(self.0, PCSTR(fn_name.as_ptr() as *const u8)) } {
            Some(ptr) => Ok(ptr),
            None => Err(windows::core::Error::from_win32()),
        }
    }
}

#[repr(transparent)]
pub(crate) struct EventObject(HANDLE);

impl Drop for EventObject {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

impl EventObject {
    /// Create a Windows Event Object for signaling encoding completion of a frame.
    pub(crate) fn new() -> windows::core::Result<Self> {
        let event = unsafe { CreateEventA(std::ptr::null(), false, false, None) }?;
        Ok(EventObject(event))
    }

    pub(crate) fn wait(&self) -> windows::core::Result<()> {
        unsafe {
            match WaitForSingleObject(self.0, INFINITE) {
                WAIT_OBJECT_0 => Ok(()),
                _ => Err(windows::core::Error::from_win32()),
            }
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.0 .0 as *mut c_void
    }
}

/// Checks if the user's NvEncAPI version is supported.
pub(crate) fn is_version_supported(lib: &Library) -> anyhow::Result<bool> {
    let mut version: u32 = 0;
    unsafe {
        let get_max_supported_version: unsafe extern "C" fn(*mut u32) -> nvenc_sys::NVENCSTATUS =
            std::mem::transmute(lib.fn_ptr("NvEncodeAPIGetMaxSupportedVersion")?);
        let status = get_max_supported_version(&mut version);
        if let Some(error) = NvEncError::new(status) {
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
pub(crate) fn get_function_list(
    lib: &Library,
) -> anyhow::Result<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST> {
    // Need to zero the struct before passing to `NvEncodeAPICreateInstance`
    let mut fn_list = MaybeUninit::<nvenc_sys::NV_ENCODE_API_FUNCTION_LIST>::zeroed();
    let fn_list = unsafe {
        // Set the version of the function list struct
        (&mut (*fn_list.as_mut_ptr())).version = nvenc_sys::NV_ENCODE_API_FUNCTION_LIST_VER;

        let create_instance: unsafe extern "C" fn(
            *mut nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        ) -> nvenc_sys::NVENCSTATUS = std::mem::transmute(lib.fn_ptr("NvEncodeAPICreateInstance")?);
        let status = create_instance(fn_list.as_mut_ptr());
        if let Some(error) = NvEncError::new(status) {
            return Err(error.into());
        }
        fn_list.assume_init()
    };

    // The function list was initialized with zero, so this should not be a null pointer when
    // the call to `NvEncodeAPICreateInstance` succeeded
    if fn_list.nvEncOpenEncodeSession.is_some() {
        Ok(fn_list)
    } else {
        Err(anyhow::anyhow!(
            "`NvEncodeAPICreateInstance` returned a malformed function list"
        ))
    }
}

/// Start an encoding session.
pub(crate) fn open_encode_session(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    device: ID3D11Device,
) -> anyhow::Result<NonNull<c_void>> {
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

    match NvEncError::new(status) {
        Some(error) => Err(error.into()),
        None => match NonNull::new(raw_encoder) {
            Some(ptr) => Ok(ptr),
            None => Err(anyhow::anyhow!(
                "`nvEncOpenEncodeSessionEx` returned a null pointer"
            )),
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
        event_params.completionEvent = event.0 .0 as *mut c_void;
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
        event_params.completionEvent = event.0 .0 as *mut c_void;
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

/// Creates an `ID3D11Texture2D` where the duplicated frames can be copied to.
pub(crate) fn create_texture_buffer(
    device: &ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
    buf_size: usize,
) -> windows::core::Result<ID3D11Texture2D> {
    let texture_desc = D3D11_TEXTURE2D_DESC {
        Width: display_desc.ModeDesc.Width,
        Height: display_desc.ModeDesc.Height,
        // plain display output has only one mip
        MipLevels: 1,
        ArraySize: buf_size as u32,
        Format: display_desc.ModeDesc.Format,
        SampleDesc: DXGI_SAMPLE_DESC {
            // default sampler mode
            Count: 1,
            // default sampler mode
            Quality: 0,
        },
        // GPU needs read/write access
        Usage: D3D11_USAGE_DEFAULT,
        // https://github.com/NVIDIA/video-sdk-samples/blob/aa3544dcea2fe63122e4feb83bf805ea40e58dbe/Samples/NvCodec/NvEncoder/NvEncoderD3D11.cpp#L90
        BindFlags: D3D11_BIND_RENDER_TARGET,
        // don't need to be accessed by the CPU
        CPUAccessFlags: D3D11_CPU_ACCESS_FLAG(0),
        MiscFlags: D3D11_RESOURCE_MISC_FLAG(0),
    };

    unsafe {
        let input_buffer = device.CreateTexture2D(&texture_desc, std::ptr::null())?;
        Ok(input_buffer)
    }
}

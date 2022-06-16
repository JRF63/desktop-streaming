use super::{nvenc_function, Result};
use crate::{
    util::{IntoNvEncBufferFormat, NvEncTexture},
    NvEncError,
};
use std::{
    mem::{ManuallyDrop, MaybeUninit},
    os::raw::c_void,
    ptr::NonNull,
};

// TODO: Make this a generic parameter
use crate::os::windows::EventObject;

pub(crate) struct NvidiaEncoderBufferItems {
    pub(crate) registered_resource: NonNull<c_void>,
    pub(crate) mapped_input: nvenc_sys::NV_ENC_INPUT_PTR,
    pub(crate) output_buffer: NonNull<c_void>,
    pub(crate) event_obj: EventObject,
}

// SAFETY: All of the struct members are pointers or pointer-like objects (`HANDLE` for the Event)
// managed by either the OS or the NvEnc API. `Send`ing them across threads would not invalidate
// them.
unsafe impl Send for NvidiaEncoderBufferItems {}

impl NvidiaEncoderBufferItems {
    pub(crate) fn new<T>(
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
        buffer_texture: &T,
        subresource_index: u32,
    ) -> Result<Self>
    where
        T: NvEncTexture,
    {
        let registered_resource =
            register_input_resource(functions, raw_encoder, buffer_texture, subresource_index)?;
        let output_buffer = create_output_buffer(functions, raw_encoder)?;

        let event_obj = EventObject::new().or(Err(NvEncError::AsyncEventCreationFailed))?;
        let registered_async = register_async_event(functions, raw_encoder, &event_obj)?;

        // All calls succeeded, remove the RAII wrappers

        let registered_resource = {
            let registered_resource = ManuallyDrop::new(registered_resource);
            registered_resource.registered_resource
        };
        let output_buffer = {
            let output_buffer = ManuallyDrop::new(output_buffer);
            output_buffer.output_buffer
        };
        let _ = ManuallyDrop::new(registered_async);

        Ok(NvidiaEncoderBufferItems {
            registered_resource,
            mapped_input: std::ptr::null_mut(),
            output_buffer,
            event_obj,
        })
    }

    pub(crate) fn cleanup(
        &mut self,
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
    ) {
        // TODO: Prob should log the errors instead of ignoring them.
        unsafe {
            (functions.nvEncUnmapInputResource.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.mapped_input,
            );
            (functions.nvEncUnregisterResource.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.registered_resource.as_ptr(),
            );
            (functions.nvEncUnlockBitstream.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.output_buffer.as_ptr(),
            );
            (functions.nvEncDestroyBitstreamBuffer.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.output_buffer.as_ptr(),
            );
            let _ignore = unregister_async_event(functions, raw_encoder, &self.event_obj);
        }
    }
}

struct RegisteredResourceRAII<'a> {
    registered_resource: NonNull<c_void>,
    functions: &'a nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
}

impl<'a> Drop for RegisteredResourceRAII<'a> {
    fn drop(&mut self) {
        unsafe {
            let _ignoring = (self.functions.nvEncUnregisterResource.unwrap_unchecked())(
                self.raw_encoder.as_ptr(),
                self.registered_resource.as_ptr(),
            );
        }
    }
}

/// Registers the passed texture for NVENC API bookkeeping.
fn register_input_resource<'a, T>(
    functions: &'a nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
    buffer_texture: &T,
    subresource_index: u32,
) -> Result<RegisteredResourceRAII<'a>>
where
    T: NvEncTexture,
{
    let (width, height, format) = buffer_texture.desc();
    let mut register_resource_params = nvenc_sys::NV_ENC_REGISTER_RESOURCE {
        version: nvenc_sys::NV_ENC_REGISTER_RESOURCE_VER,
        resourceType: T::resource_type(),
        width,
        height,
        pitch: 0,
        subResourceIndex: subresource_index,
        resourceToRegister: buffer_texture.as_ptr(),
        registeredResource: std::ptr::null_mut(),
        bufferFormat: format.into_nvenc_buffer_format(),
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
    // Should not fail since `nvEncRegisterResource` succeeded
    let registered_resource =
        NonNull::new(register_resource_params.registeredResource).ok_or(NvEncError::Generic)?;

    Ok(RegisteredResourceRAII {
        registered_resource,
        functions,
        raw_encoder,
    })
}

struct OutputBufferRAII<'a> {
    output_buffer: NonNull<c_void>,
    functions: &'a nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
}

impl<'a> Drop for OutputBufferRAII<'a> {
    fn drop(&mut self) {
        unsafe {
            let _ignoring = (self
                .functions
                .nvEncDestroyBitstreamBuffer
                .unwrap_unchecked())(
                self.raw_encoder.as_ptr(), self.output_buffer.as_ptr()
            );
        }
    }
}

/// Allocate an output buffer. Should be called only after the encoder has been configured.
fn create_output_buffer<'a>(
    functions: &'a nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
) -> Result<OutputBufferRAII<'a>> {
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
    // Should not fail since `nvEncCreateBitstreamBuffer` succeeded
    let output_buffer =
        NonNull::new(create_bitstream_buffer_params.bitstreamBuffer).ok_or(NvEncError::Generic)?;
    Ok(OutputBufferRAII {
        output_buffer,
        functions,
        raw_encoder,
    })
}

struct AsyncEventRAII<'a, 'b> {
    event: &'a EventObject,
    functions: &'b nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
}

impl<'a, 'b> Drop for AsyncEventRAII<'a, 'b> {
    fn drop(&mut self) {
        let _ignoring = unregister_async_event(self.functions, self.raw_encoder, self.event);
    }
}

fn register_async_event<'a, 'b>(
    functions: &'b nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
    event_obj: &'a EventObject,
) -> Result<AsyncEventRAII<'a, 'b>> {
    #[cfg(windows)]
    unsafe {
        let mut event_params: nvenc_sys::NV_ENC_EVENT_PARAMS = MaybeUninit::zeroed().assume_init();
        event_params.version = nvenc_sys::NV_ENC_EVENT_PARAMS_VER;
        event_params.completionEvent = event_obj.as_ptr();
        nvenc_function!(
            functions.nvEncRegisterAsyncEvent,
            raw_encoder.as_ptr(),
            &mut event_params
        );
    }
    Ok(AsyncEventRAII {
        event: event_obj,
        functions,
        raw_encoder,
    })
}

fn unregister_async_event(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    raw_encoder: NonNull<c_void>,
    event_obj: &EventObject,
) -> Result<()> {
    #[cfg(windows)]
    unsafe {
        let mut event_params: nvenc_sys::NV_ENC_EVENT_PARAMS = MaybeUninit::zeroed().assume_init();
        event_params.version = nvenc_sys::NV_ENC_EVENT_PARAMS_VER;
        event_params.completionEvent = event_obj.as_ptr();
        nvenc_function!(
            functions.nvEncUnregisterAsyncEvent,
            raw_encoder.as_ptr(),
            &mut event_params
        );
    }
    Ok(())
}

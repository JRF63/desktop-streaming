use super::{EventObject, EventObjectTrait, RawEncoder, Result};
use crate::{
    util::{IntoNvEncBufferFormat, NvEncTexture},
    NvEncError,
};
use std::{
    mem::{ManuallyDrop, MaybeUninit},
    os::raw::c_void,
    ptr::NonNull,
};

pub struct NvidiaEncoderBufferItems {
    pub registered_resource: NonNull<c_void>,
    pub mapped_input: crate::sys::NV_ENC_INPUT_PTR,
    pub output_buffer: NonNull<c_void>,
    pub event_obj: EventObject,
    pub end_of_stream: bool,
}

// SAFETY: All of the struct members are pointers or pointer-like objects (`HANDLE` for the Event)
// managed by either the OS or the NvEnc API. `Send`ing them across threads would not invalidate
// them.
unsafe impl Send for NvidiaEncoderBufferItems {}

impl NvidiaEncoderBufferItems {
    pub fn new<T>(
        raw_encoder: &RawEncoder,
        buffer_texture: &T,
        subresource_index: u32,
    ) -> Result<Self>
    where
        T: NvEncTexture,
    {
        let registered_resource =
            register_input_resource(raw_encoder, buffer_texture, subresource_index)?;
        let output_buffer = create_output_buffer(raw_encoder)?;

        let event_obj = EventObject::new()?;
        let registered_async = register_async_event(raw_encoder, &event_obj)?;

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
            end_of_stream: false,
        })
    }

    pub fn cleanup(&mut self, raw_encoder: &RawEncoder) {
        // TODO: Prob should log the errors instead of ignoring them.
        unsafe {
            let _ = raw_encoder.unmap_input_resource(self.mapped_input);
            let _ = raw_encoder.unregister_resource(self.registered_resource.as_ptr());
            let _ = raw_encoder.unlock_bitstream(self.output_buffer.as_ptr());
            let _ = raw_encoder.destroy_bitstream_buffer(self.output_buffer.as_ptr());
        }
        let _ignore = unregister_async_event(raw_encoder, &self.event_obj);
    }
}

struct RegisteredResourceRAII<'a> {
    registered_resource: NonNull<c_void>,
    raw_encoder: &'a RawEncoder,
}

impl<'a> Drop for RegisteredResourceRAII<'a> {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .raw_encoder
                .unregister_resource(self.registered_resource.as_ptr());
        }
    }
}

/// Registers the passed texture for NVENC API bookkeeping.
fn register_input_resource<'a, T>(
    raw_encoder: &'a RawEncoder,
    buffer_texture: &T,
    subresource_index: u32,
) -> Result<RegisteredResourceRAII<'a>>
where
    T: NvEncTexture,
{
    let (width, height, format) = buffer_texture.desc();
    let mut register_resource_params = crate::sys::NV_ENC_REGISTER_RESOURCE {
        version: crate::sys::NV_ENC_REGISTER_RESOURCE_VER,
        resourceType: T::resource_type(),
        width,
        height,
        pitch: 0,
        subResourceIndex: subresource_index,
        resourceToRegister: buffer_texture.as_ptr(),
        registeredResource: std::ptr::null_mut(),
        bufferFormat: format.into_nvenc_buffer_format(),
        bufferUsage: crate::sys::NV_ENC_BUFFER_USAGE::NV_ENC_INPUT_IMAGE,
        pInputFencePoint: std::ptr::null_mut(),
        pOutputFencePoint: std::ptr::null_mut(),
        reserved1: [0; 247],
        reserved2: [std::ptr::null_mut(); 60],
    };

    unsafe {
        raw_encoder.register_resource(&mut register_resource_params)?;
    }

    // Should not fail since `nvEncRegisterResource` succeeded
    let registered_resource =
        NonNull::new(register_resource_params.registeredResource).ok_or(NvEncError::default())?;

    Ok(RegisteredResourceRAII {
        registered_resource,
        raw_encoder,
    })
}

struct OutputBufferRAII<'a> {
    output_buffer: NonNull<c_void>,
    raw_encoder: &'a RawEncoder,
}

impl<'a> Drop for OutputBufferRAII<'a> {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .raw_encoder
                .destroy_bitstream_buffer(self.output_buffer.as_ptr());
        }
    }
}

/// Allocate an output buffer. Should be called only after the encoder has been configured.
fn create_output_buffer<'a>(raw_encoder: &'a RawEncoder) -> Result<OutputBufferRAII<'a>> {
    let mut create_bitstream_buffer_params: crate::sys::NV_ENC_CREATE_BITSTREAM_BUFFER =
        unsafe { MaybeUninit::zeroed().assume_init() };
    create_bitstream_buffer_params.version = crate::sys::NV_ENC_CREATE_BITSTREAM_BUFFER_VER;

    unsafe {
        raw_encoder.create_bitstream_buffer(&mut create_bitstream_buffer_params)?;
    }

    // Should not fail since `nvEncCreateBitstreamBuffer` succeeded
    let output_buffer =
        NonNull::new(create_bitstream_buffer_params.bitstreamBuffer).ok_or(NvEncError::default())?;
    Ok(OutputBufferRAII {
        output_buffer,
        raw_encoder,
    })
}

struct AsyncEventRAII<'a, 'b> {
    event_obj: &'a EventObject,
    raw_encoder: &'b RawEncoder,
}

impl<'a, 'b> Drop for AsyncEventRAII<'a, 'b> {
    fn drop(&mut self) {
        let _ignoring = unregister_async_event(self.raw_encoder, self.event_obj);
    }
}

fn register_async_event<'a, 'b>(
    raw_encoder: &'b RawEncoder,
    event_obj: &'a EventObject,
) -> Result<AsyncEventRAII<'a, 'b>> {
    #[cfg(windows)]
    unsafe {
        let mut event_params: crate::sys::NV_ENC_EVENT_PARAMS = MaybeUninit::zeroed().assume_init();
        event_params.version = crate::sys::NV_ENC_EVENT_PARAMS_VER;
        event_params.completionEvent = event_obj.as_ptr();
        raw_encoder.register_async_event(&mut event_params)?;
    }
    Ok(AsyncEventRAII {
        event_obj,
        raw_encoder,
    })
}

fn unregister_async_event(raw_encoder: &RawEncoder, event_obj: &EventObject) -> Result<()> {
    #[cfg(windows)]
    unsafe {
        let mut event_params: crate::sys::NV_ENC_EVENT_PARAMS = MaybeUninit::zeroed().assume_init();
        event_params.version = crate::sys::NV_ENC_EVENT_PARAMS_VER;
        event_params.completionEvent = event_obj.as_ptr();
        raw_encoder.unregister_async_event(&mut event_params)?;
    }
    Ok(())
}

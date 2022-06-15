mod config;
mod init;
mod input;
mod output;
mod queries;

use self::init::*;
use crate::{nvenc_function, sync::CyclicBuffer, Codec, EncoderPreset, Result, TuningInfo};
use config::EncoderParams;
use input::EncoderInput;
use output::EncoderOutput;
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull, sync::Arc};
use windows::Win32::Graphics::{
    Direct3D11::{ID3D11Device, ID3D11Texture2D},
    Dxgi::DXGI_OUTDUPL_DESC,
};

use crate::os::windows::{create_texture_buffer, EventObject, Library};

const BUFFER_SIZE: usize = 8;

pub(crate) struct EncoderBuffers {
    registered_resource: NonNull<c_void>,
    input_ptr: nvenc_sys::NV_ENC_INPUT_PTR,
    output_ptr: NonNull<c_void>,
    event_obj: EventObject,
}

unsafe impl Sync for EncoderBuffers {}

impl EncoderBuffers {
    pub(crate) fn cleanup(
        &mut self,
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
    ) {
        // TODO: Prob should log the errors instead of ignoring them.
        unsafe {
            (functions.nvEncUnmapInputResource.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.input_ptr,
            );
            (functions.nvEncUnregisterResource.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.registered_resource.as_ptr(),
            );
            (functions.nvEncDestroyBitstreamBuffer.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.output_ptr.as_ptr(),
            );
            let _ignore = unregister_async_event(functions, raw_encoder, &self.event_obj);
        }
    }
}

pub(crate) struct NvidiaEncoder {
    raw_encoder: NonNull<c_void>,
    functions: nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    input_textures: ID3D11Texture2D,

    buffer: CyclicBuffer<EncoderBuffers, BUFFER_SIZE>,

    #[allow(dead_code)]
    library: Library,
}

impl Drop for NvidiaEncoder {
    fn drop(&mut self) {
        for buffer in self.buffer.get_mut() {
            buffer.get_mut().cleanup(&self.functions, self.raw_encoder);
        }
        unsafe {
            (self.functions.nvEncDestroyEncoder.unwrap())(self.raw_encoder.as_ptr());
        }
    }
}

// TODO: `Sync` and `Send` are technically wrong
unsafe impl Sync for NvidiaEncoder {}
unsafe impl Send for NvidiaEncoder {}

impl NvidiaEncoder {
    pub(crate) fn new(
        device: ID3D11Device,
        display_desc: &DXGI_OUTDUPL_DESC,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> anyhow::Result<(Self, EncoderParams)> {
        let library = Library::load("nvEncodeAPI64.dll")?;
        if !is_version_supported(&library)? {
            return Err(anyhow::anyhow!(
                "NVENC version is not supported by the installed driver"
            ));
        }
        let functions = get_function_list(&library)?;
        let raw_encoder = open_encode_session(&functions, &device)?;

        let mut encoder_params = EncoderParams::new(
            &functions,
            raw_encoder,
            display_desc,
            codec,
            preset,
            tuning_info,
        )?;
        unsafe {
            nvenc_function!(
                functions.nvEncInitializeEncoder,
                raw_encoder.as_ptr(),
                encoder_params.init_params_mut()
            );
        }

        let input_textures = create_texture_buffer(&device, display_desc, BUFFER_SIZE)?;

        // Using a closure for graceful cleanup
        let inner = || -> anyhow::Result<[EncoderBuffers; BUFFER_SIZE]> {
            let mut buffer = MaybeUninit::<[EncoderBuffers; BUFFER_SIZE]>::uninit();
            unsafe {
                // Pointer to the start of the array's buffer
                let mut ptr = (&mut *buffer.as_mut_ptr()).as_mut_ptr();
                for i in 0..BUFFER_SIZE {
                    let registered_resource = register_resource(
                        &functions,
                        raw_encoder,
                        input_textures.clone(),
                        i as u32,
                    )?;
                    let output_ptr = create_output_buffers(&functions, raw_encoder)?;
                    let event_obj = EventObject::new()?;
                    register_async_event(&functions, raw_encoder, &event_obj)?;
                    ptr.write(EncoderBuffers {
                        registered_resource,
                        input_ptr: std::ptr::null_mut(),
                        output_ptr,
                        event_obj,
                    });
                    ptr = ptr.offset(1);
                }
                Ok(buffer.assume_init())
            }
        };

        // if let Err(e) = inner() {
        //     for mut buffer in buffers {
        //         buffer.get_mut().cleanup(&functions, raw_encoder);
        //     }
        //     return Err(e);
        // }
        let buffer = inner()?;

        Ok((
            NvidiaEncoder {
                raw_encoder,
                functions,
                input_textures,
                buffer: CyclicBuffer::new(buffer),
                library,
            },
            encoder_params,
        ))
    }
}

pub fn create_encoder(
    device: ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
    codec: Codec,
    preset: EncoderPreset,
    tuning_info: TuningInfo,
) -> (EncoderInput, EncoderOutput) {
    let mut device_context = None;
    unsafe {
        device.GetImmediateContext(&mut device_context);
    }

    let (encoder, encoder_params) =
        NvidiaEncoder::new(device, display_desc, codec, preset, tuning_info).unwrap();
    let encoder = Arc::new(encoder);

    let encoder_input = EncoderInput::new(
        encoder.clone(),
        device_context.unwrap(),
        encoder_params,
    );

    let encoder_output = EncoderOutput::new(encoder);

    (encoder_input, encoder_output)
}

mod config;
mod init;
mod input;
mod output;
mod queries;

use self::init::*;
use crate::{nvenc_function, Codec, EncoderPreset, Result, TuningInfo};
use config::EncoderParams;
use input::{EncoderInput, EncoderInputReturn};
use output::EncoderOutput;
use std::{
    cell::UnsafeCell,
    os::raw::c_void,
    ptr::NonNull,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use windows::Win32::Graphics::{
    Direct3D11::{ID3D11Device, ID3D11Texture2D},
    Dxgi::DXGI_OUTDUPL_DESC,
};

use crate::os::windows::{create_texture_buffer, EventObject, Library};

pub(crate) struct EncoderBuffers {
    registered_resource: NonNull<c_void>,
    input_ptr: UnsafeCell<nvenc_sys::NV_ENC_INPUT_PTR>,
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
                *self.input_ptr.get(),
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

    /// Index of the writer
    head: AtomicUsize,
    /// Index of the reader
    tail: AtomicUsize,
    /// Buffer
    buffers: Vec<EncoderBuffers>,

    #[allow(dead_code)]
    library: Library,
}

impl Drop for NvidiaEncoder {
    fn drop(&mut self) {
        for buffer in &mut self.buffers {
            buffer.cleanup(&self.functions, self.raw_encoder);
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
        buf_size: usize,
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

        let input_textures = create_texture_buffer(&device, display_desc, buf_size)?;

        let mut buffers = Vec::with_capacity(buf_size);

        // Using a closure for graceful cleanup
        let mut inner = || -> anyhow::Result<()> {
            for i in 0..buf_size {
                let registered_resource =
                    register_resource(&functions, raw_encoder, input_textures.clone(), i as u32)?;
                let output_ptr = create_output_buffers(&functions, raw_encoder)?;
                let event_obj = EventObject::new()?;
                register_async_event(&functions, raw_encoder, &event_obj)?;

                buffers.push(EncoderBuffers {
                    registered_resource,
                    input_ptr: UnsafeCell::new(std::ptr::null_mut()),
                    output_ptr,
                    event_obj,
                });
            }
            Ok(())
        };

        if let Err(e) = inner() {
            for mut buffer in buffers {
                buffer.cleanup(&functions, raw_encoder);
            }
            return Err(e);
        }

        Ok((
            NvidiaEncoder {
                raw_encoder,
                functions,
                input_textures,
                head: AtomicUsize::new(0),
                tail: AtomicUsize::new(0),
                buffers,
                library,
            },
            encoder_params,
        ))
    }

    /// Modify an item on the buffer. Blocks if the buffer is full.
    #[inline]
    pub(super) fn modify<F>(&self, mut modify_op: F)
    where
        F: FnMut(&mut EncoderBuffers),
    {
        // `CyclicBuffer` is purposely not `Send` - the value that will be read here is from a
        // previous `Ordering::Release` store by the same thread
        let head = self.head.load(Ordering::Relaxed);
        loop {
            let tail = self.tail.load(Ordering::Acquire);

            // Break if not full
            if (head - tail) <= self.buffers.len() {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = head & (self.buffers.len() - 1);
        unsafe {
            let cell = self.buffers.get_unchecked(index);
            // modify_op(&mut *cell.get());
        }

        self.head.store(head + 1, Ordering::Release);
    }

    /// Read an item on the buffer. Blocks if the buffer is empty.
    #[inline]
    pub(super) fn read<F>(&self, mut read_op: F)
    where
        F: FnMut(&EncoderBuffers),
    {
        // `Ordering::Relaxed` has the same reasoning as on `modify`
        let tail = self.tail.load(Ordering::Relaxed);
        loop {
            let head = self.head.load(Ordering::Acquire);

            // Break if not empty
            if head != tail {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = tail & (self.buffers.len() - 1);
        unsafe {
            let cell = self.buffers.get_unchecked(index);
            // read_op(&*cell.get());
        }

        self.tail.store(tail + 1, Ordering::Release);
    }
}

pub fn create_encoder(
    device: ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
    codec: Codec,
    preset: EncoderPreset,
    tuning_info: TuningInfo,
    buf_size: usize,
) -> (EncoderInput, EncoderOutput) {
    let mut device_context = None;
    unsafe {
        device.GetImmediateContext(&mut device_context);
    }

    let (encoder, encoder_params) =
        NvidiaEncoder::new(device, display_desc, codec, preset, tuning_info, buf_size).unwrap();
    let encoder = Arc::new(encoder);

    let EncoderInputReturn {
        encoder_input,
        avail_indices_sender,
        occupied_indices_receiver,
    } = EncoderInput::new(
        encoder.clone(),
        device_context.unwrap(),
        encoder_params,
        buf_size,
    );

    let encoder_output =
        EncoderOutput::new(encoder, occupied_indices_receiver, avail_indices_sender);

    (encoder_input, encoder_output)
}

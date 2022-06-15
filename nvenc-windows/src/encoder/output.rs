use super::NvidiaEncoder;
use crate::nvenc_function;
use anyhow::Context;
use crossbeam_channel::{Receiver, Sender};
use std::{mem::MaybeUninit, sync::Arc};

pub struct EncoderOutput {
    encoder: Arc<NvidiaEncoder>,
}

impl EncoderOutput {
    pub(crate) fn new(
        encoder: Arc<NvidiaEncoder>,
    ) -> Self {
        EncoderOutput {
            encoder,
        }
    }

    pub fn wait_for_output<F: FnMut(&nvenc_sys::NV_ENC_LOCK_BITSTREAM) -> ()>(
        &self,
        mut consume_output: F,
    ) -> anyhow::Result<()> {
        const ERR_LABEL: &'static str = "EncoderOutput error";

        self.encoder.buffer.reader_access(|buffer| -> anyhow::Result<()> {
            buffer.event_obj.blocking_wait().context(ERR_LABEL)?;

            let mut lock_params: nvenc_sys::NV_ENC_LOCK_BITSTREAM =
                unsafe { MaybeUninit::zeroed().assume_init() };
            lock_params.version = nvenc_sys::NV_ENC_LOCK_BITSTREAM_VER;
            lock_params.outputBitstream = buffer.output_ptr.as_ptr();

            unsafe {
                nvenc_function!(
                    self.encoder.functions.nvEncLockBitstream,
                    self.encoder.raw_encoder.as_ptr(),
                    &mut lock_params
                );
            }

            consume_output(&lock_params);

            unsafe {
                nvenc_function!(
                    self.encoder.functions.nvEncUnlockBitstream,
                    self.encoder.raw_encoder.as_ptr(),
                    lock_params.outputBitstream
                );

                nvenc_function!(
                    self.encoder.functions.nvEncUnmapInputResource,
                    self.encoder.raw_encoder.as_ptr(),
                    buffer.input_ptr
                );
            }
            Ok(())
        })
    }
}

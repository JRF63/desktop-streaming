use super::NvidiaEncoderShared;
use anyhow::Context;
use std::{mem::MaybeUninit, sync::Arc};

pub struct EncoderOutput<const BUF_SIZE: usize> {
    shared: Arc<NvidiaEncoderShared<BUF_SIZE>>,
}

impl<const BUF_SIZE: usize> EncoderOutput<BUF_SIZE> {
    pub(crate) fn new(shared: Arc<NvidiaEncoderShared<BUF_SIZE>>) -> Self {
        EncoderOutput { shared }
    }

    pub fn wait_for_output<F: FnMut(&nvenc_sys::NV_ENC_LOCK_BITSTREAM) -> ()>(
        &self,
        mut consume_output: F,
    ) -> anyhow::Result<()> {
        const ERR_LABEL: &'static str = "EncoderOutput error";

        self.shared
            .buffer
            .reader_access(|buffer| -> anyhow::Result<()> {
                buffer.event_obj.blocking_wait().context(ERR_LABEL)?;

                let mut lock_params: nvenc_sys::NV_ENC_LOCK_BITSTREAM =
                    unsafe { MaybeUninit::zeroed().assume_init() };
                lock_params.version = nvenc_sys::NV_ENC_LOCK_BITSTREAM_VER;
                lock_params.outputBitstream = buffer.output_buffer.as_ptr();

                unsafe {
                    self.shared.raw_encoder.lock_bitstream(&mut lock_params)?;
                }

                consume_output(&lock_params);

                unsafe {
                    self.shared
                        .raw_encoder
                        .unlock_bitstream(lock_params.outputBitstream)?;
                    self.shared
                        .raw_encoder
                        .unmap_input_resource(buffer.mapped_input)?;
                }

                Ok(())
            })
    }
}

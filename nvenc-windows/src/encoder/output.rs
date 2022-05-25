use super::{NvidiaEncoder, Result};
use crate::nvenc_function;
use crossbeam_channel::{Receiver, Sender};
use std::{mem::MaybeUninit, sync::Arc};

pub struct EncoderOutput {
    encoder: Arc<NvidiaEncoder>,
    occupied_indices_receiver: Receiver<usize>,
    avail_indices_sender: Sender<usize>,
}

impl EncoderOutput {
    pub(crate) fn new(
        encoder: Arc<NvidiaEncoder>,
        occupied_indices_receiver: Receiver<usize>,
        avail_indices_sender: Sender<usize>,
    ) -> Self {
        EncoderOutput {
            encoder,
            occupied_indices_receiver,
            avail_indices_sender,
        }
    }

    pub fn wait_for_output<F: FnMut(&nvenc_sys::NV_ENC_LOCK_BITSTREAM) -> ()>(
        &self,
        mut consume_output: F,
    ) -> Result<()> {
        // TODO: Handle `recv` error
        let index = self.occupied_indices_receiver.recv().unwrap();
        // TODO: Handle `wait` error
        self.encoder.buffers[index].event_obj.wait().unwrap();
        let mut lock_params: nvenc_sys::NV_ENC_LOCK_BITSTREAM =
            unsafe { MaybeUninit::zeroed().assume_init() };
        lock_params.version = nvenc_sys::NV_ENC_LOCK_BITSTREAM_VER;
        lock_params.outputBitstream = self.encoder.buffers[index].output_ptr.as_ptr();

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
                *self.encoder.buffers[index].input_ptr.get()
            );
        }
        // TODO: Handle `try_send` error
        self.avail_indices_sender.try_send(index).unwrap();
        Ok(())
    }
}

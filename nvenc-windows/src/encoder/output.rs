use super::{NvidiaEncoder, Result};
use crate::nvenc_function;
use crossbeam_channel::{Receiver, Sender};
use std::sync::Arc;
use windows::Win32::System::{
    Threading::{WaitForSingleObject, WAIT_OBJECT_0},
    WindowsProgramming::INFINITE,
};

pub struct EncoderOutput<const BUF_SIZE: usize> {
    encoder: Arc<NvidiaEncoder<BUF_SIZE>>,
    occupied_indices: Receiver<usize>,
    avail_indices: Sender<usize>,
}

impl<const BUF_SIZE: usize> EncoderOutput<BUF_SIZE> {
    pub fn wait_for_output<F: FnMut(&nvenc_sys::NV_ENC_LOCK_BITSTREAM) -> ()>(
        &self,
        mut consume_output: F,
    ) -> Result<()> {
        // TODO: Handle `recv` error
        let index = self.occupied_indices.recv().unwrap();
        match unsafe { WaitForSingleObject(self.encoder.io[index].event_obj, INFINITE) } {
            WAIT_OBJECT_0 => {
                let mut lock_params: nvenc_sys::NV_ENC_LOCK_BITSTREAM =
                    unsafe { std::mem::zeroed() };
                lock_params.version = nvenc_sys::NV_ENC_LOCK_BITSTREAM_VER;
                lock_params.outputBitstream = self.encoder.io[index].output_ptr.as_ptr();

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
                        self.encoder.io[index].input_ptr
                    );
                }
            }
            _ => panic!("Waiting for event object failed."), // TODO: Handle error
        }
        // TODO: Handle `try_send` error
        self.avail_indices.try_send(index).unwrap();
        Ok(())
    }
}

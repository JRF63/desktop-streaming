use super::{shared::NvidiaEncoderReader, event::EventObjectTrait};
use crate::{NvEncError, Result};
use std::mem::MaybeUninit;

pub struct EncoderOutput {
    reader: NvidiaEncoderReader,
}

impl EncoderOutput {
    pub(crate) fn new(reader: NvidiaEncoderReader) -> Self {
        EncoderOutput { reader }
    }

    pub fn wait_for_output<F: FnMut(&crate::sys::NV_ENC_LOCK_BITSTREAM) -> ()>(
        &self,
        mut consume_output: F,
    ) -> Result<()> {
        const TIMEOUT_INTERVAL_MS: u32 = 1000; // Wait for 1 sec
        self.reader.read(|buffer| -> Result<()> {
            buffer.event_obj.wait(TIMEOUT_INTERVAL_MS)?;

            if buffer.end_of_stream {
                return Err(NvEncError::EndOfStream);
            }

            let mut lock_params: crate::sys::NV_ENC_LOCK_BITSTREAM =
                unsafe { MaybeUninit::zeroed().assume_init() };
            lock_params.version = crate::sys::NV_ENC_LOCK_BITSTREAM_VER;
            lock_params.outputBitstream = buffer.output_buffer.as_ptr();

            unsafe {
                self.reader.lock_bitstream(&mut lock_params)?;
            }

            consume_output(&lock_params);

            unsafe {
                self.reader.unlock_bitstream(lock_params.outputBitstream)?;
                self.reader.unmap_input_resource(buffer.mapped_input)?;
            }

            Ok(())
        })
    }
}

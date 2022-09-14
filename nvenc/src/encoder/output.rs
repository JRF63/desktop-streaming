use super::{EventObjectTrait, NvidiaEncoderReader, ENCODER_BUFFER_SIZE};
use crate::Result;
use std::mem::MaybeUninit;

pub struct EncoderOutput {
    reader: NvidiaEncoderReader<ENCODER_BUFFER_SIZE>,
}

impl EncoderOutput {
    pub fn new(reader: NvidiaEncoderReader<ENCODER_BUFFER_SIZE>) -> Self {
        EncoderOutput { reader }
    }

    pub fn wait_for_output<F: FnMut(&crate::sys::NV_ENC_LOCK_BITSTREAM) -> ()>(
        &self,
        mut consume_output: F,
    ) -> Result<()> {
        self.reader.read(|buffer| -> Result<()> {
            buffer.event_obj.wait()?;

            // End of input stream
            if buffer.mapped_input.is_null() {
                return Ok(());
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

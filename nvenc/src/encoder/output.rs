use super::NvidiaEncoderReader;
use anyhow::Context;
use std::mem::MaybeUninit;

pub struct EncoderOutput<const N: usize> {
    reader: NvidiaEncoderReader<N>,
}

impl<const N: usize> EncoderOutput<N> {
    pub(crate) fn new(reader: NvidiaEncoderReader<N>) -> Self {
        EncoderOutput { reader }
    }

    pub fn wait_for_output<F: FnMut(&nvenc_sys::NV_ENC_LOCK_BITSTREAM) -> ()>(
        &self,
        mut consume_output: F,
    ) -> anyhow::Result<()> {
        const ERR_LABEL: &'static str = "EncoderOutput error";

        self.reader.read(|buffer| -> anyhow::Result<()> {
            buffer.event_obj.blocking_wait().context(ERR_LABEL)?;

            let mut lock_params: nvenc_sys::NV_ENC_LOCK_BITSTREAM =
                unsafe { MaybeUninit::zeroed().assume_init() };
            lock_params.version = nvenc_sys::NV_ENC_LOCK_BITSTREAM_VER;
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

mod sync;

use super::{
    buffer_items::EncoderBufferItems, builder::BUFFER_SIZE, raw_encoder::RawEncoder,
    texture::TextureBufferImplTrait,
};
use crate::Result;
use std::{mem::MaybeUninit, ops::Deref, sync::Arc};
use sync::{CyclicBuffer, CyclicBufferReader, CyclicBufferWriter};

struct NvidiaEncoderShared {
    raw_encoder: RawEncoder,
    buffer: CyclicBuffer<EncoderBufferItems, BUFFER_SIZE>,
}

impl Drop for NvidiaEncoderShared {
    fn drop(&mut self) {
        for buffer in self.buffer.get_mut() {
            buffer.get_mut().cleanup(&self.raw_encoder);
        }
    }
}

pub fn encoder_channel<T>(
    raw_encoder: RawEncoder,
    texture_buffer: &T,
) -> Result<(NvidiaEncoderWriter, NvidiaEncoderReader)>
where
    T: TextureBufferImplTrait,
{
    let buffer = unsafe {
        let mut buffer = MaybeUninit::<[EncoderBufferItems; BUFFER_SIZE]>::uninit();

        // Pointer to the start of the array's buffer
        let mut ptr = (&mut *buffer.as_mut_ptr()).as_mut_ptr();

        for i in 0..BUFFER_SIZE {
            ptr.write(EncoderBufferItems::new(
                &raw_encoder,
                texture_buffer.get_texture(i),
                texture_buffer.get_pitch_or_subresource_index(i),
            )?);
            ptr = ptr.offset(1);
        }
        buffer.assume_init()
    };

    let shared_encoder = Arc::new(NvidiaEncoderShared {
        raw_encoder,
        buffer: CyclicBuffer::new(buffer).unwrap(), // TODO: remove unwrap
    });
    let writer = NvidiaEncoderWriter(shared_encoder.clone());
    let reader = NvidiaEncoderReader(shared_encoder);

    Ok((writer, reader))
}

#[repr(transparent)]
pub struct NvidiaEncoderWriter(Arc<NvidiaEncoderShared>);

// Writes to `NvidiaEncoderWriter` are synchronized with reads from `NvidiaEncoderReader` but only
// if there is exactly one reader and one writer
unsafe impl Send for NvidiaEncoderWriter {}

impl NvidiaEncoderWriter {
    /// Modify an item on the buffer. Blocks if the buffer is full.
    #[inline]
    pub fn write<F, R>(&self, write_op: F) -> R
    where
        F: FnOnce(usize, &mut EncoderBufferItems) -> R,
    {
        let writer = unsafe { CyclicBufferWriter::from_shared_buffer(&self.0.buffer) };
        writer.write(write_op)
    }
}

#[repr(transparent)]
pub struct NvidiaEncoderReader(Arc<NvidiaEncoderShared>);

// Reads to `NvidiaEncoderReader` are synchronized with writes from `NvidiaEncoderWriter` but only
// if there is exactly one reader and one writer
unsafe impl Send for NvidiaEncoderReader {}

impl NvidiaEncoderReader {
    /// Read an item on the buffer. Blocks if the buffer is empty.
    #[inline]
    pub fn read<F, R>(&self, read_op: F) -> R
    where
        F: FnOnce(&EncoderBufferItems) -> R,
    {
        let reader = unsafe { CyclicBufferReader::from_shared_buffer(&self.0.buffer) };
        reader.read(read_op)
    }
}

// TODO: Limit what methods are available to `NvidiaEncoderWriter` instead of blanket enabling
// all methods of `RawEncoder`
impl Deref for NvidiaEncoderWriter {
    type Target = RawEncoder;

    fn deref(&self) -> &Self::Target {
        &self.0.raw_encoder
    }
}

// TODO: Ditto `NvidiaEncoderWriter`'s `Deref`
impl Deref for NvidiaEncoderReader {
    type Target = RawEncoder;

    fn deref(&self) -> &Self::Target {
        &self.0.raw_encoder
    }
}

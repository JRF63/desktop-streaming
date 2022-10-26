mod sync;

use super::{buffer::NvidiaEncoderBufferItems, EncoderParams, RawEncoder};
use crate::{
    util::{NvEncDevice, NvEncTexture},
    Codec, EncodePreset, Result, TuningInfo,
};
use std::{mem::MaybeUninit, ops::Deref, sync::Arc};
use sync::{CyclicBuffer, CyclicBufferReader, CyclicBufferWriter};

use windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_DESC;

/// Size of the ring buffer that is shared between the input and output
pub const ENCODER_BUFFER_SIZE: usize = 8;

struct NvidiaEncoderShared {
    raw_encoder: RawEncoder,
    buffer: CyclicBuffer<NvidiaEncoderBufferItems, ENCODER_BUFFER_SIZE>,
}

impl Drop for NvidiaEncoderShared {
    fn drop(&mut self) {
        for buffer in self.buffer.get_mut() {
            buffer.get_mut().cleanup(&self.raw_encoder);
        }
    }
}

pub fn encoder_channel<D, T>(
    device: &D,
    display_desc: &DXGI_OUTDUPL_DESC,
    buffer_texture: &T,
    codec: Codec,
    preset: EncodePreset,
    tuning_info: TuningInfo,
) -> Result<(
    (NvidiaEncoderWriter, NvidiaEncoderReader),
    EncoderParams,
)>
where
    D: NvEncDevice,
    T: NvEncTexture,
{
    let raw_encoder = RawEncoder::new(device)?;

    let mut encoder_params =
        EncoderParams::new(&raw_encoder, display_desc, codec, preset, tuning_info)?;

    unsafe {
        raw_encoder.initialize_encoder(encoder_params.init_params_mut())?;
    }

    let buffer = unsafe {
        let mut buffer = MaybeUninit::<[NvidiaEncoderBufferItems; ENCODER_BUFFER_SIZE]>::uninit();

        // Pointer to the start of the array's buffer
        let mut ptr = (&mut *buffer.as_mut_ptr()).as_mut_ptr();

        for i in 0..ENCODER_BUFFER_SIZE {
            ptr.write(NvidiaEncoderBufferItems::new(
                &raw_encoder,
                buffer_texture,
                i as u32,
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

    Ok(((writer, reader), encoder_params))
}

#[repr(transparent)]
pub struct NvidiaEncoderWriter(Arc<NvidiaEncoderShared>);

// Writes to `NvidiaEncoderWriter` are synchronized with reads from `NvidiaEncoderReader` but only
// if there is exactly one reader and one writer
unsafe impl Send for NvidiaEncoderWriter {}

impl NvidiaEncoderWriter {
    /// Modify an item on the buffer. Blocks if the buffer is full.
    #[inline]
    pub fn write<F, S, R>(&self, args: S, write_op: F) -> R
    where
        F: FnMut(usize, &mut NvidiaEncoderBufferItems, S) -> R,
    {
        let writer = unsafe { CyclicBufferWriter::from_shared_buffer(&self.0.buffer) };
        writer.write(args, write_op)
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
        F: FnMut(&NvidiaEncoderBufferItems) -> R,
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

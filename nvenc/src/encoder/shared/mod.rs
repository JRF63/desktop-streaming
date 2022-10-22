mod sync;

use super::{buffer::NvidiaEncoderBufferItems, EncoderParams, RawEncoder};
use crate::{
    util::{NvEncDevice, NvEncTexture},
    Codec, EncodePreset, Result, TuningInfo,
};
use std::{mem::MaybeUninit, ops::Deref, sync::Arc};
use sync::{CyclicBuffer, CyclicBufferReader, CyclicBufferWriter};

use windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_DESC;

struct NvidiaEncoderShared<const N: usize> {
    raw_encoder: RawEncoder,
    buffer: CyclicBuffer<NvidiaEncoderBufferItems, N>,
}

impl<const N: usize> Drop for NvidiaEncoderShared<N> {
    fn drop(&mut self) {
        for buffer in self.buffer.get_mut() {
            buffer.get_mut().cleanup(&self.raw_encoder);
        }
    }
}

pub fn encoder_channel<const N: usize, D, T>(
    device: &D,
    display_desc: &DXGI_OUTDUPL_DESC,
    buffer_texture: &T,
    codec: Codec,
    preset: EncodePreset,
    tuning_info: TuningInfo,
) -> Result<(
    (NvidiaEncoderWriter<N>, NvidiaEncoderReader<N>),
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
        let mut buffer = MaybeUninit::<[NvidiaEncoderBufferItems; N]>::uninit();

        // Pointer to the start of the array's buffer
        let mut ptr = (&mut *buffer.as_mut_ptr()).as_mut_ptr();

        for i in 0..N {
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
pub struct NvidiaEncoderWriter<const N: usize>(Arc<NvidiaEncoderShared<N>>);

// Writes to `NvidiaEncoderWriter` are synchronized with reads from `NvidiaEncoderReader` but only
// if there is exactly one reader and one writer
unsafe impl<const N: usize> Send for NvidiaEncoderWriter<N> {}

impl<const N: usize> NvidiaEncoderWriter<N> {
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
pub struct NvidiaEncoderReader<const N: usize>(Arc<NvidiaEncoderShared<N>>);

// Reads to `NvidiaEncoderReader` are synchronized with writes from `NvidiaEncoderWriter` but only
// if there is exactly one reader and one writer
unsafe impl<const N: usize> Send for NvidiaEncoderReader<N> {}

impl<const N: usize> NvidiaEncoderReader<N> {
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
impl<const N: usize> Deref for NvidiaEncoderWriter<N> {
    type Target = RawEncoder;

    fn deref(&self) -> &Self::Target {
        &self.0.raw_encoder
    }
}

// TODO: Ditto `NvidiaEncoderWriter`'s `Deref`
impl<const N: usize> Deref for NvidiaEncoderReader<N> {
    type Target = RawEncoder;

    fn deref(&self) -> &Self::Target {
        &self.0.raw_encoder
    }
}

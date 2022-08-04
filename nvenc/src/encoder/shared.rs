use super::{buffer::NvidiaEncoderBufferItems, EncoderParams, RawEncoder};
use crate::{
    sync::CyclicBuffer,
    util::{NvEncDevice, NvEncTexture},
    Codec, EncoderPreset, TuningInfo,
};
use std::mem::MaybeUninit;

use windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_DESC;

pub(crate) struct NvidiaEncoderShared<const N: usize> {
    pub(crate) raw_encoder: RawEncoder,
    pub(crate) buffer: CyclicBuffer<NvidiaEncoderBufferItems, N>,
}

impl<const N: usize> Drop for NvidiaEncoderShared<N> {
    fn drop(&mut self) {
        for buffer in self.buffer.get_mut() {
            buffer.get_mut().cleanup(&self.raw_encoder);
        }
    }
}

impl<const N: usize> NvidiaEncoderShared<N> {
    pub(crate) fn new<D, T>(
        device: &D,
        display_desc: &DXGI_OUTDUPL_DESC,
        buffer_texture: &T,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> anyhow::Result<(Self, EncoderParams)>
    where
        D: NvEncDevice,
        T: NvEncTexture,
    {
        assert!(
            N.count_ones() == 1,
            "Buffer size must be a power of two"
        );

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

        Ok((
            NvidiaEncoderShared {
                raw_encoder,
                buffer: CyclicBuffer::new(buffer),
            },
            encoder_params,
        ))
    }
}

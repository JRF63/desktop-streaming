use crate::AudioFormatType;
use std::ptr::NonNull;
use windows::Win32::{
    Media::{
        Audio::{IAudioClient, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM},
        KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE},
        Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT},
    },
    System::Com::{CoTaskMemAlloc, CoTaskMemFree},
};

pub struct AudioFormat {
    inner: NonNull<WAVEFORMATEX>,
    block_align: usize,
}

impl Drop for AudioFormat {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.inner.as_ptr().cast()));
        }
    }
}

impl AudioFormat {
    /// Get the audio format that a device uses for its internal processing of shared-mode streams.
    pub fn get_mix_format(audio_client: &IAudioClient) -> Result<Self, windows::core::Error> {
        unsafe {
            audio_client
                .GetMixFormat()
                .and_then(|ptr| match NonNull::new(ptr) {
                    Some(non_null) => {
                        let block_align = non_null.as_ref().nBlockAlign as usize;
                        Ok(Self {
                            inner: non_null,
                            block_align,
                        })
                    }
                    None => Err(windows::core::Error::from_win32()),
                })
        }
    }

    /// Create an audio format that the Opus encoder can encode.
    pub fn opus_encoder_input_format(sampling_rate: u32) -> Result<Self, windows::core::Error> {
        const NUM_BITS_PER_BYTE: u32 = 8;

        let channels: u32 = crate::NUM_CHANNELS;
        let bits_per_sample: u32 = crate::OPUS_BITS_PER_SAMPLE;
        let block_align = channels * bits_per_sample / NUM_BITS_PER_BYTE;
        let avg_bytes_per_sec = sampling_rate * block_align;

        unsafe {
            let ptr: *mut WAVEFORMATEX = CoTaskMemAlloc(std::mem::size_of::<WAVEFORMATEX>()).cast();
            match NonNull::new(ptr) {
                Some(mut non_null) => {
                    let wave_format = non_null.as_mut();

                    *wave_format = WAVEFORMATEX {
                        wFormatTag: WAVE_FORMAT_PCM as u16,
                        nChannels: channels as u16,
                        nSamplesPerSec: sampling_rate,
                        nAvgBytesPerSec: avg_bytes_per_sec,
                        nBlockAlign: block_align as u16,
                        wBitsPerSample: bits_per_sample as u16,
                        cbSize: 0,
                    };

                    Ok(Self {
                        inner: non_null,
                        block_align: block_align as usize,
                    })
                }
                None => Err(windows::core::Error::from_win32()),
            }
        }
    }

    fn get_format_tag(&self) -> u16 {
        unsafe { (*self.inner.as_ptr()).wFormatTag }
    }

    pub fn as_wave_format(&self) -> &WAVEFORMATEX {
        unsafe { self.inner.as_ref() }
    }

    pub fn as_extensible_format(&self) -> Option<&WAVEFORMATEXTENSIBLE> {
        unsafe {
            if self.get_format_tag() == WAVE_FORMAT_EXTENSIBLE as u16 {
                Some(self.inner.cast().as_ref())
            } else {
                None
            }
        }
    }

    pub fn audio_format_type(&self) -> Option<AudioFormatType> {
        match self.as_extensible_format() {
            Some(ext_format) => unsafe {
                let sub_format_ptr = std::ptr::addr_of!(ext_format.SubFormat);
                match std::ptr::read_unaligned(sub_format_ptr) {
                    KSDATAFORMAT_SUBTYPE_PCM => Some(AudioFormatType::Pcm),
                    KSDATAFORMAT_SUBTYPE_IEEE_FLOAT => Some(AudioFormatType::IeeeFloat),
                    _ => None,
                }
            },
            None => match self.get_format_tag() as u32 {
                WAVE_FORMAT_PCM => Some(AudioFormatType::Pcm),
                WAVE_FORMAT_IEEE_FLOAT => Some(AudioFormatType::IeeeFloat),
                _ => None,
            },
        }
    }

    pub fn get_sampling_rate(&self) -> u32 {
        unsafe { (*self.inner.as_ptr()).nSamplesPerSec }
    }

    pub fn get_block_align(&self) -> usize {
        self.block_align
    }
}

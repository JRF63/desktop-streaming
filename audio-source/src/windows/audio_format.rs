use crate::audio_data::AudioFormatKind;
use std::{mem::MaybeUninit, ptr::NonNull};
use windows::Win32::{
    Foundation::{S_FALSE, S_OK},
    Media::{
        Audio::{
            IAudioClient, AUDCLNT_E_UNSUPPORTED_FORMAT, AUDCLNT_SHAREMODE, WAVEFORMATEX,
            WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM,
        },
        KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE},
        Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT},
    },
    System::Com::{CoTaskMemAlloc, CoTaskMemFree},
};

const OPUS_SAMPLING_RATES: [u32; 5] = [8000, 12000, 16000, 24000, 48000];
const OPUS_MAX_AUDIO_CHANNELS: u16 = 2;
const OPUS_BITS_PER_SAMPLE: u32 = 16;

pub struct AudioFormat {
    inner: NonNull<WAVEFORMATEX>,
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
                    Some(non_null) => Ok(Self { inner: non_null }),
                    None => Err(windows::core::Error::from_win32()),
                })
        }
    }

    /// Create an audio format for 16-bit signed PCM that the Opus encoder can encode.
    pub fn opus_encoder_input_format(sampling_rate: u32) -> Result<Self, windows::core::Error> {
        const NUM_BITS_PER_BYTE: u32 = 8;

        let channels = OPUS_MAX_AUDIO_CHANNELS;
        let bits_per_sample: u32 = OPUS_BITS_PER_SAMPLE;
        let block_align = channels as u32 * bits_per_sample / NUM_BITS_PER_BYTE;
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

                    Ok(Self { inner: non_null })
                }
                None => Err(windows::core::Error::from_win32()),
            }
        }
    }

    pub fn is_supported_by_device(
        &self,
        audio_client: &IAudioClient,
        share_mode: AUDCLNT_SHAREMODE,
    ) -> Result<bool, windows::core::Error> {
        unsafe {
            let mut closest_match = MaybeUninit::uninit();
            let hresult = audio_client.IsFormatSupported(
                share_mode,
                self.as_wave_format(),
                Some(closest_match.as_mut_ptr()),
            );

            let closest_match = closest_match.assume_init();
            if !closest_match.is_null() {
                CoTaskMemFree(Some(closest_match.cast()));
            }

            match hresult {
                S_OK => Ok(true),
                S_FALSE => Ok(false),
                e => {
                    if e == AUDCLNT_E_UNSUPPORTED_FORMAT {
                        Ok(false)
                    } else {
                        Err(windows::core::Error::from_win32())
                    }
                }
            }
        }
    }

    fn format_tag(&self) -> u16 {
        unsafe { self.inner.as_ref().wFormatTag }
    }

    pub fn as_wave_format(&self) -> &WAVEFORMATEX {
        unsafe { self.inner.as_ref() }
    }

    fn as_extensible_format(&self) -> Option<&WAVEFORMATEXTENSIBLE> {
        unsafe {
            if self.format_tag() == WAVE_FORMAT_EXTENSIBLE as u16 {
                Some(self.inner.cast().as_ref())
            } else {
                None
            }
        }
    }

    pub fn audio_format_kind(&self) -> Option<AudioFormatKind> {
        match self.as_extensible_format() {
            Some(ext_format) => unsafe {
                let sub_format_ptr = std::ptr::addr_of!(ext_format.SubFormat);
                match std::ptr::read_unaligned(sub_format_ptr) {
                    KSDATAFORMAT_SUBTYPE_PCM => Some(AudioFormatKind::Pcm),
                    KSDATAFORMAT_SUBTYPE_IEEE_FLOAT => Some(AudioFormatKind::IeeeFloat),
                    _ => None,
                }
            },
            None => match self.format_tag() as u32 {
                WAVE_FORMAT_PCM => Some(AudioFormatKind::Pcm),
                WAVE_FORMAT_IEEE_FLOAT => Some(AudioFormatKind::IeeeFloat),
                _ => None,
            },
        }
    }

    pub fn sampling_rate(&self) -> u32 {
        unsafe { self.inner.as_ref().nSamplesPerSec }
    }

    pub fn is_sampling_rate_supported_by_encoder(&self) -> bool {
        OPUS_SAMPLING_RATES.contains(&self.sampling_rate())
    }

    /// Set the sampling rate to one supported by the Opus encoder. Does nothing if the current
    /// sampling rate is already supported.
    pub fn fix_sampling_rate_if_unsupported(&mut self) {
        if !self.is_sampling_rate_supported_by_encoder() {
            unsafe {
                self.inner.as_mut().nSamplesPerSec = 48000;
            }
        }
    }

    pub fn num_channels(&self) -> u16 {
        unsafe { self.inner.as_ref().nChannels }
    }

    /// If the current number of audio channels exceeds the max supported by the Opus encoder, set
    /// it to the max supported instead.
    pub fn fix_num_audio_channels_if_unsupported(&mut self) {
        if self.num_channels() > OPUS_MAX_AUDIO_CHANNELS {
            unsafe {
                self.inner.as_mut().nChannels = OPUS_MAX_AUDIO_CHANNELS;
            }
        }
    }
}

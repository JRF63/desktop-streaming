use std::mem::MaybeUninit;
use windows::{
    core::HSTRING,
    Win32::{
        Media::Audio::{
            eConsole, eRender, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
            AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
            AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, WAVEFORMATEX, WAVE_FORMAT_PCM,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
        },
    },
};

pub struct AudioCapture {
    audio_client: IAudioClient,
    device_id: String,
}

impl AudioCapture {
    pub fn new(device_id: Option<String>) -> Result<Self, windows::core::Error> {
        // TODO: Is multithreading this correct?
        let threading_model = COINIT_MULTITHREADED;

        // COM usage is in the same process
        let class_context = CLSCTX_INPROC_SERVER;

        unsafe {
            CoInitializeEx(None, threading_model)?;

            let device_enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, class_context)?;

            let (device, device_id) = match device_id {
                Some(device_id) => (
                    device_enumerator.GetDevice(&HSTRING::from(&device_id))?,
                    device_id,
                ),
                None => {
                    let dataflow = eRender;
                    let role = eConsole;
                    let device = device_enumerator.GetDefaultAudioEndpoint(dataflow, role)?;

                    // ToString::to_string works too
                    let device_id = device.GetId()?.to_hstring()?.to_string_lossy();

                    (device, device_id)
                }
            };

            let audio_client: IAudioClient = device.Activate(class_context, None)?;

            Ok(Self {
                audio_client,
                device_id,
            })
        }
    }

    fn opus_pcm_input_format() -> WAVEFORMATEX {
        const NUM_BITS_PER_BYTE: u32 = 8;

        let channels: u32 = crate::NUM_CHANNELS;
        let samples_per_sec: u32 = crate::OPUS_BITRATE;
        let bits_per_sample: u32 = crate::OPUS_BITS_PER_SAMPLE;
        let block_align = channels * bits_per_sample / NUM_BITS_PER_BYTE;
        let avg_bytes_per_sec = samples_per_sec * block_align;

        WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: channels as u16,
            nSamplesPerSec: samples_per_sec,
            nAvgBytesPerSec: avg_bytes_per_sec,
            nBlockAlign: block_align as u16,
            wBitsPerSample: bits_per_sample as u16,
            cbSize: 0,
        }
    }

    pub fn initialize(&self) -> Result<(), windows::core::Error> {
        let mut default_device_period = MaybeUninit::uninit(); // Not going to be used
        let mut min_device_period = MaybeUninit::uninit();

        unsafe {
            self.audio_client.GetDevicePeriod(
                Some(default_device_period.as_mut_ptr()),
                Some(min_device_period.as_mut_ptr()),
            )?;

            let min_device_period = min_device_period.assume_init();

            let share_mode = AUDCLNT_SHAREMODE_SHARED;
            let stream_flags = AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
                | AUDCLNT_STREAMFLAGS_EVENTCALLBACK;

            let capture_format = AudioCapture::opus_pcm_input_format();

            self.audio_client.Initialize(
                share_mode,
                stream_flags,
                min_device_period,
                0,
                &capture_format,
                None,
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_capture_init() {
        let audio_capture = AudioCapture::new(None).unwrap();
        audio_capture.initialize().unwrap();
    }
}

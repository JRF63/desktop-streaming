use std::{
    mem::MaybeUninit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
};
use windows::{
    core::HSTRING,
    Win32::{
        Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Media::Audio::{
            eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
            MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
            AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, WAVEFORMATEX, WAVE_FORMAT_PCM,
        },
        System::{
            Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            Threading::{AvSetMmThreadCharacteristicsA, CreateEventA, WaitForSingleObject},
        },
    },
};

const WAIT_INTERVAL_MS: u32 = 100;

pub struct AudioCapture {
    audio_client: IAudioClient,
    buffer_event: HANDLE,
    running: Arc<AtomicBool>,
    device_id: String,
    format: WAVEFORMATEX,
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}

impl AudioCapture {
    pub fn new(device_id: Option<String>) -> Result<Self, windows::core::Error> {
        // COM usage is in the same process
        let class_context = CLSCTX_INPROC_SERVER;

        unsafe {
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

                    // `ToString::to_string` works too
                    let device_id = device.GetId()?.to_hstring()?.to_string_lossy();

                    (device, device_id)
                }
            };

            let audio_client: IAudioClient = device.Activate(class_context, None)?;

            Ok(Self {
                audio_client,
                buffer_event: CreateEventA(None, false, false, None)?,
                running: Arc::new(AtomicBool::new(false)),
                device_id,
                format: AudioCapture::opus_pcm_input_format(),
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

    fn meow(
        capture_client: IAudioCaptureClient,
        buffer_event: HANDLE,
        block_align: usize,
        running: Arc<AtomicBool>,
    ) -> Result<(), windows::core::Error> {
        while running.load(Ordering::Acquire) {
            unsafe {
                match WaitForSingleObject(buffer_event, WAIT_INTERVAL_MS) {
                    WAIT_OBJECT_0 => {
                        let mut frames_available = capture_client.GetNextPacketSize()?;
                        let data_size = block_align * frames_available as usize;
                        println!("data_size: {data_size}");

                        let mut data = MaybeUninit::uninit();
                        let mut flags = MaybeUninit::uninit();
                        let mut device_position = MaybeUninit::uninit();
                        let mut qpc_position = MaybeUninit::uninit();

                        capture_client.GetBuffer(
                            data.as_mut_ptr(),
                            &mut frames_available,
                            flags.as_mut_ptr(),
                            Some(device_position.as_mut_ptr()),
                            Some(qpc_position.as_mut_ptr()),
                        )?;

                        capture_client.ReleaseBuffer(frames_available)?;
                    }
                    WAIT_TIMEOUT => continue,
                    _ => return Err(windows::core::Error::from_win32()), // Last error
                }
            }
        }
        Ok(())
    }

    pub fn start(&self) -> Result<(), windows::core::Error> {
        let mut default_device_period = MaybeUninit::uninit(); // Not going to be used
        let mut min_device_period = MaybeUninit::uninit();

        unsafe {
            self.audio_client.GetDevicePeriod(
                Some(default_device_period.as_mut_ptr()),
                Some(min_device_period.as_mut_ptr()),
            )?;

            let min_device_period = min_device_period.assume_init();
            let periodicity = 0; // Must be 0 when `AUDCLNT_SHAREMODE_SHARED`

            let share_mode = AUDCLNT_SHAREMODE_SHARED;
            let stream_flags = AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
                | AUDCLNT_STREAMFLAGS_EVENTCALLBACK;

            self.audio_client.Initialize(
                share_mode,
                stream_flags,
                min_device_period,
                periodicity,
                &self.format,
                None,
            )?;

            self.audio_client.SetEventHandle(self.buffer_event)?;

            self.audio_client.Start()?;
            self.running.store(true, Ordering::Release);


            let join_handle = AudioCapture::meow(
                self.audio_client.GetService()?,
                self.buffer_event,
                self.format.nBlockAlign as usize,
                self.running.clone(),
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    #[test]
    fn test_audio_capture_init() {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).unwrap();
        }

        let audio_capture = AudioCapture::new(None).unwrap();
        audio_capture.start().unwrap();

        unsafe {
            CoUninitialize();
        }
    }
}

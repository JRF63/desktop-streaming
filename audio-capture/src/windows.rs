use roundabout_buffer::{RoundaboutBuffer, RoundaboutBufferReader, RoundaboutBufferWriter};
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
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                COINIT_MULTITHREADED,
            },
            Threading::{
                AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventA,
                WaitForSingleObject,
            },
        },
    },
};

const WAIT_INTERVAL_MS: u32 = 100;
const NUM_BUFFERS: usize = 4;
const PRO_AUDIO_TASK_NAME: &str = "Pro Audio";
const APPROX_AUDIO_DATA_LEN: usize = 1920;

pub struct AudioData {
    data: Vec<u8>,
    flags: u32,
    timestamp: u64,
}

impl AudioData {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(APPROX_AUDIO_DATA_LEN),
            flags: 0,
            timestamp: 0,
        }
    }
}

pub struct AudioCapture {
    reader: RoundaboutBufferReader<AudioData, NUM_BUFFERS>,
    running: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<Result<(), windows::core::Error>>>,
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        let _ = self.join_handle.take().and_then(|x| x.join().ok());
    }
}

impl AudioCapture {
    pub fn new(device_id: Option<String>) -> Self {
        // TODO: Vec::with_capacity
        let buffer = std::array::from_fn(|_| AudioData::new());

        let (writer, reader) = RoundaboutBuffer::channel(buffer);
        let running = Arc::new(AtomicBool::new(false));
        Self {
            reader,
            running: running.clone(),
            join_handle: Some(AudioCapture::spawn_thread(device_id, writer, running)),
        }
    }

    pub fn recv_audio_data<F>(&mut self, mut read_op: F)
    where
        F: FnMut(&AudioData),
    {
        self.reader.read(|_, buffer| read_op(buffer))
    }

    fn spawn_thread(
        device_id: Option<String>,
        mut writer: RoundaboutBufferWriter<AudioData, NUM_BUFFERS>,
        running: Arc<AtomicBool>,
    ) -> JoinHandle<Result<(), windows::core::Error>> {
        running.store(true, Ordering::Release);
        thread::spawn(move || -> Result<(), windows::core::Error> {
            let mut task_index = MaybeUninit::uninit();
            let handle = unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED)?;
                AvSetMmThreadCharacteristicsW(
                    &HSTRING::from(PRO_AUDIO_TASK_NAME),
                    task_index.as_mut_ptr(),
                )?
            };

            let audio_client = AudioClient::new(device_id)?;
            let capture_client = audio_client.start()?;

            while running.load(Ordering::Acquire) {
                audio_client.send_audio_data(&capture_client, &mut writer)?;
            }

            unsafe {
                AvRevertMmThreadCharacteristics(handle)?;
                CoUninitialize();
            }

            Ok(())
        })
    }
}

pub struct AudioClient {
    audio_client: IAudioClient,
    buffer_event: HANDLE,
    audio_format: WAVEFORMATEX,
    device_id: String,
}

impl AudioClient {
    fn new(device_id: Option<String>) -> Result<Self, windows::core::Error> {
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
                audio_format: opus_pcm_input_format(),
                device_id,
            })
        }
    }

    fn start(&self) -> Result<IAudioCaptureClient, windows::core::Error> {
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
                &self.audio_format,
                None,
            )?;

            self.audio_client.SetEventHandle(self.buffer_event)?;

            self.audio_client.Start()?;

            self.audio_client.GetService()
        }
    }

    fn send_audio_data(
        &self,
        capture_client: &IAudioCaptureClient,
        writer: &mut RoundaboutBufferWriter<AudioData, NUM_BUFFERS>,
    ) -> Result<(), windows::core::Error> {
        unsafe {
            match WaitForSingleObject(self.buffer_event, WAIT_INTERVAL_MS) {
                WAIT_OBJECT_0 => {
                    let mut frames_available = capture_client.GetNextPacketSize()?;
                    let block_align = self.audio_format.nBlockAlign as usize;
                    let data_size = block_align * frames_available as usize;

                    let mut data = MaybeUninit::uninit();
                    let mut flags = MaybeUninit::uninit();
                    let mut qpc_position = MaybeUninit::uninit();

                    capture_client.GetBuffer(
                        data.as_mut_ptr(),
                        &mut frames_available,
                        flags.as_mut_ptr(),
                        None,
                        Some(qpc_position.as_mut_ptr()),
                    )?;

                    writer.write(|_, audio_data| {
                        audio_data.data.clear();
                        audio_data
                            .data
                            .extend_from_slice(std::slice::from_raw_parts(
                                data.assume_init(),
                                data_size,
                            ));
                        audio_data.flags = flags.assume_init();
                        audio_data.timestamp = qpc_position.assume_init();
                    });

                    capture_client.ReleaseBuffer(frames_available)?;
                }
                WAIT_TIMEOUT => (),
                _ => return Err(windows::core::Error::from_win32()), // Last error
            }
        }
        Ok(())
    }

    fn get_device_id(&self) -> &str {
        &self.device_id
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_capture_init() {
        let mut audio_capture = AudioCapture::new(None);
        // audio_capture.start().unwrap();
        let now = std::time::Instant::now();
        while now.elapsed() < std::time::Duration::from_secs(10) {
            audio_capture.recv_audio_data(|audio_data| {
                println!("flags: {} qpc: {}", audio_data.flags, audio_data.timestamp);
            })
        }
    }
}

mod com_thread;
mod thread_priority;

use self::{
    com_thread::ComThread,
    thread_priority::{ThreadPriority, ThreadProfile},
};
use crate::util::ExtendFromBytes;
use conveyor_buffer::{ConveyorBufferReader, ConveyorBufferWriter};
use std::{
    mem::MaybeUninit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
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
            Com::{CoCreateInstance, CLSCTX, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED},
            Threading::{CreateEventA, WaitForSingleObject},
        },
    },
};

const WAIT_INTERVAL_MS: u32 = 100;
const NUM_BUFFERS: usize = 64;
const APPROX_AUDIO_DATA_LEN: usize = 1920;

pub struct AudioData {
    data: Vec<i16>,
    frames_available: u32,
    flags: u32,
    timestamp: u64,
}

impl AudioData {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(APPROX_AUDIO_DATA_LEN),
            frames_available: 0,
            flags: 0,
            timestamp: 0,
        }
    }
}

pub struct AudioCapturer {
    reader: ConveyorBufferReader<AudioData, NUM_BUFFERS>,
    running: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<Result<(), windows::core::Error>>>,
    device_id: Arc<Mutex<Option<String>>>,
}

impl Drop for AudioCapturer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        let _ = self.join_handle.take().and_then(|x| x.join().ok());
    }
}

impl AudioCapturer {
    pub fn new(device_id: Option<String>) -> Self {
        let buffer = std::array::from_fn(|_| AudioData::new());

        let (writer, reader) = conveyor_buffer::channel(buffer);
        let running = Arc::new(AtomicBool::new(false));

        let device_id = Arc::new(Mutex::new(device_id));

        Self {
            reader,
            running: running.clone(),
            join_handle: Some(AudioCapturer::spawn_thread(
                device_id.clone(),
                writer,
                running,
            )),
            device_id,
        }
    }

    pub fn recv_audio_data<F>(&mut self, mut read_op: F)
    where
        F: FnMut(&AudioData),
    {
        self.reader.read(|_, buffer| read_op(buffer))
    }

    pub fn get_device_id(&self) -> Option<String> {
        let mutex_guard = self.device_id.lock().expect("Mutex was poisoned");
        mutex_guard.clone()
    }

    fn spawn_thread(
        device_id: Arc<Mutex<Option<String>>>,
        mut writer: ConveyorBufferWriter<AudioData, NUM_BUFFERS>,
        running: Arc<AtomicBool>,
    ) -> JoinHandle<Result<(), windows::core::Error>> {
        running.store(true, Ordering::Release);
        thread::spawn(move || -> Result<(), windows::core::Error> {
            // Leave option for refactoring to multithreaded access
            let thread_model = COINIT_MULTITHREADED;
            // COM usage is in the same process
            let class_context = CLSCTX_INPROC_SERVER;
            // Minimizes latency
            let thread_profile = ThreadProfile::ProAudio;

            let _priority = ThreadPriority::new(thread_profile)?;
            let _com = ComThread::new(thread_model)?;

            let device_enumerator: IMMDeviceEnumerator =
                unsafe { CoCreateInstance(&MMDeviceEnumerator, None, class_context)? };

            let capture_client =
                AudioCaptureClient::new(&device_enumerator, device_id, class_context)?;

            while running.load(Ordering::Acquire) {
                capture_client.send_audio_data(&mut writer)?;
            }

            Ok(())
        })
    }
}

struct AudioCaptureClient {
    inner: IAudioCaptureClient,
    buffer_event: HANDLE,
    audio_format: WAVEFORMATEX,
}

impl AudioCaptureClient {
    pub fn new(
        device_enumerator: &IMMDeviceEnumerator,
        device_id: Arc<Mutex<Option<String>>>,
        class_context: CLSCTX,
    ) -> Result<Self, windows::core::Error> {
        unsafe {
            let device = {
                let mut mutex_guard = device_id.lock().expect("Mutex was poisoned");
                match mutex_guard.as_deref() {
                    Some(device_id) => device_enumerator.GetDevice(&HSTRING::from(device_id))?,
                    None => {
                        let dataflow = eRender;
                        let role = eConsole;
                        let device = device_enumerator.GetDefaultAudioEndpoint(dataflow, role)?;

                        // `ToString::to_string` works too
                        *mutex_guard = Some(device.GetId()?.to_hstring()?.to_string_lossy());

                        device
                    }
                }
            };

            let audio_client: IAudioClient = device.Activate(class_context, None)?;
            let buffer_event = CreateEventA(None, false, false, None)?;
            let audio_format = opus_pcm_input_format();

            let mut default_device_period = MaybeUninit::uninit(); // Not going to be used
            let mut min_device_period = MaybeUninit::uninit();

            audio_client.GetDevicePeriod(
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

            audio_client.Initialize(
                share_mode,
                stream_flags,
                min_device_period,
                periodicity,
                &audio_format,
                None,
            )?;

            audio_client.SetEventHandle(buffer_event)?;

            audio_client.Start()?;

            Ok(Self {
                inner: audio_client.GetService()?,
                buffer_event,
                audio_format,
            })
        }
    }

    fn send_audio_data(
        &self,
        writer: &mut ConveyorBufferWriter<AudioData, NUM_BUFFERS>,
    ) -> Result<(), windows::core::Error> {
        unsafe {
            match WaitForSingleObject(self.buffer_event, WAIT_INTERVAL_MS) {
                WAIT_OBJECT_0 => {
                    let mut frames_available = self.inner.GetNextPacketSize()?;
                    let block_align = self.audio_format.nBlockAlign as usize;
                    let data_size = block_align * frames_available as usize;

                    let mut data = MaybeUninit::uninit();
                    let mut flags = MaybeUninit::uninit();
                    let mut qpc_position = MaybeUninit::uninit();

                    self.inner.GetBuffer(
                        data.as_mut_ptr(),
                        &mut frames_available,
                        flags.as_mut_ptr(),
                        None,
                        Some(qpc_position.as_mut_ptr()),
                    )?;

                    writer.write(|_, audio_data| {
                        let bytes = std::slice::from_raw_parts(data.assume_init(), data_size);
                        let (prefix, mid, suffix) = bytes.align_to::<i16>();

                        audio_data.data.clear();
                        audio_data.data.extend_from_bytes(prefix);
                        audio_data.data.extend_from_slice(mid);
                        audio_data.data.extend_from_bytes(suffix);

                        audio_data.frames_available = frames_available;
                        audio_data.flags = flags.assume_init();
                        audio_data.timestamp = qpc_position.assume_init();
                    });

                    // Must call `ReleaseBuffer` for every successful `GetBuffer`
                    self.inner.ReleaseBuffer(frames_available)?;
                }
                WAIT_TIMEOUT => (),
                _ => return Err(windows::core::Error::from_win32()), // Last error
            }
        }
        Ok(())
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
        let mut audio_capture = AudioCapturer::new(None);

        let now = std::time::Instant::now();
        while now.elapsed() < std::time::Duration::from_secs(3) {
            audio_capture.recv_audio_data(|audio_data| {
                println!("flags: {} qpc: {}", audio_data.flags, audio_data.timestamp);
            })
        }
    }
}

mod audio_format;
mod com_thread;
mod thread_priority;

use self::{
    audio_format::AudioFormat,
    com_thread::ComThread,
    thread_priority::{ThreadPriority, ThreadProfile},
};
use crate::AudioFormatType;
use conveyor_buffer::{ConveyorBufferReader, ConveyorBufferWriter};
use serde::{Deserialize, Serialize};
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
            eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
            MMDeviceEnumerator, AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
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
const POISONED_MUTEX_MSG: &str = "Mutex was poisoned";

#[derive(Clone, Serialize, Deserialize)]
pub struct AudioData {
    pub data: Vec<i16>,
    pub num_frames: u32,
    pub flags: u32,
    pub timestamp: u64,
}

impl AudioData {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(APPROX_AUDIO_DATA_LEN),
            num_frames: 0,
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
            join_handle: Some(BackgroundAudioCapturer::spawn(
                device_id.clone(),
                writer,
                running,
            )),
            device_id,
        }
    }

    pub fn get_audio_data(&mut self) -> impl std::ops::Deref<Target = AudioData> + '_ {
        self.reader.get().1
    }

    pub fn get_device_id(&self) -> Option<String> {
        self.device_id.lock().expect(POISONED_MUTEX_MSG).clone()
    }
}

struct BackgroundAudioCapturer {
    audio_capture_client: IAudioCaptureClient,
    buffer_event: HANDLE,
    audio_format: AudioFormat,
    audio_format_type: AudioFormatType,

    // TODO: Useful for setting the volume
    #[allow(dead_code)]
    audio_client: IAudioClient,
}

impl BackgroundAudioCapturer {
    fn spawn(
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

            while running.load(Ordering::Acquire) {
                let capture_client = BackgroundAudioCapturer::new(
                    &device_enumerator,
                    device_id.clone(),
                    class_context,
                )?;

                while running.load(Ordering::Acquire) {
                    if let Err(e) = capture_client.send_audio_data(&mut writer) {
                        // Handle a removed audio device by querying for the new default device
                        if e.code() == AUDCLNT_E_DEVICE_INVALIDATED {
                            *device_id.lock().expect(POISONED_MUTEX_MSG) = None;
                            break;
                        } else {
                            return Err(e);
                        }
                    }
                }
            }

            Ok(())
        })
    }

    fn new(
        device_enumerator: &IMMDeviceEnumerator,
        device_id: Arc<Mutex<Option<String>>>,
        class_context: CLSCTX,
    ) -> Result<Self, windows::core::Error> {
        let device = BackgroundAudioCapturer::get_source_device(device_enumerator, &device_id)?;
        unsafe {
            let audio_client: IAudioClient = device.Activate(class_context, None)?;

            // `AUDCLNT_SHAREMODE_SHARED` because the audio is just being duplicated and has to be
            // shared with others.
            // `AUDCLNT_STREAMFLAGS_LOOPBACK` is to signal audio duplication.
            // `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` is used because events will be used in waiting
            // for audio data.
            let share_mode = AUDCLNT_SHAREMODE_SHARED;
            let mut stream_flags = AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK;

            let mut audio_format = AudioFormat::get_mix_format(&audio_client)?;

            let audio_format_type = match audio_format.audio_format_type() {
                Some(t) => {
                    let mut audio_format_type = t;
                    
                    if !audio_format.is_sampling_rate_supported_by_encoder() {
                        audio_format.set_sampling_rate_to_supported();
                        if !audio_format.is_supported_by_device(&audio_client, share_mode)? {
                            stream_flags |= AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
                            audio_format = AudioFormat::opus_encoder_input_format(
                                audio_format.get_sampling_rate(),
                            )?;
                            audio_format_type = AudioFormatType::Pcm;
                        }
                    }
                    audio_format_type
                }
                None => {
                    // Format is unsupported by the encoder so auto-convert it to a high-quality
                    // PCM stream.
                    stream_flags |= AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                        | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
                    audio_format =
                        AudioFormat::opus_encoder_input_format(audio_format.get_sampling_rate())?;
                    AudioFormatType::Pcm
                }
            };

            let buffer_duration = {
                let mut default_device_period = MaybeUninit::uninit(); // Not going to be used
                let mut min_device_period = MaybeUninit::uninit();

                audio_client.GetDevicePeriod(
                    Some(default_device_period.as_mut_ptr()),
                    Some(min_device_period.as_mut_ptr()),
                )?;
                min_device_period.assume_init()
            };

            // Must be 0 when `AUDCLNT_SHAREMODE_SHARED`
            let periodicity = 0;

            audio_client.Initialize(
                share_mode,
                stream_flags,
                buffer_duration,
                periodicity,
                audio_format.as_wave_format(),
                None,
            )?;

            let buffer_event = CreateEventA(None, false, false, None)?;

            audio_client.SetEventHandle(buffer_event)?;

            audio_client.Start()?;

            Ok(Self {
                audio_capture_client: audio_client.GetService()?,
                buffer_event,
                audio_format,
                audio_format_type,
                audio_client,
            })
        }
    }

    /// Get an audio rendering device (i.e, headphones, speakers) from which the audio will be
    /// duplicated from.
    fn get_source_device(
        device_enumerator: &IMMDeviceEnumerator,
        device_id: &Arc<Mutex<Option<String>>>,
    ) -> Result<IMMDevice, windows::core::Error> {
        let mut mutex_guard = match device_id.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::error!("{}", e);
                panic!("{}", e);
            }
        };
        unsafe {
            match mutex_guard.as_deref() {
                Some(device_id) => Ok(device_enumerator.GetDevice(&HSTRING::from(device_id))?),
                None => {
                    let dataflow = eRender;
                    let role = eConsole;
                    let device = device_enumerator.GetDefaultAudioEndpoint(dataflow, role)?;

                    *mutex_guard = match device.GetId()?.to_string() {
                        Ok(s) => Some(s),
                        Err(_) => None, // Unlikely error
                    };

                    Ok(device)
                }
            }
        }
    }

    fn send_audio_data(
        &self,
        writer: &mut ConveyorBufferWriter<AudioData, NUM_BUFFERS>,
    ) -> Result<(), windows::core::Error> {
        unsafe {
            match WaitForSingleObject(self.buffer_event, WAIT_INTERVAL_MS) {
                WAIT_OBJECT_0 => {
                    let mut frames_available = self.audio_capture_client.GetNextPacketSize()?;
                    let num_bytes = self.audio_format.get_block_align() * frames_available as usize;

                    let mut data = MaybeUninit::uninit();
                    let mut flags = MaybeUninit::uninit();
                    let mut qpc_position = MaybeUninit::uninit();

                    self.audio_capture_client.GetBuffer(
                        data.as_mut_ptr(),
                        &mut frames_available,
                        flags.as_mut_ptr(),
                        None,
                        Some(qpc_position.as_mut_ptr()),
                    )?;

                    writer.write(|_, audio_data| {
                        // The `u8` array is half the size when converted to an `i16` array
                        let pcm_data_len = num_bytes / 2;

                        audio_data.data.clear();
                        audio_data.data.reserve(pcm_data_len);

                        // This should be safe since an `i16` array is also aligned to a `u8`
                        // array
                        std::ptr::copy_nonoverlapping(
                            data.assume_init(),
                            audio_data.data.as_mut_ptr().cast(),
                            num_bytes,
                        );

                        audio_data.data.set_len(pcm_data_len);

                        audio_data.num_frames = frames_available;
                        audio_data.flags = flags.assume_init();
                        audio_data.timestamp = qpc_position.assume_init();
                    });

                    // Must call `ReleaseBuffer` for every successful `GetBuffer`
                    self.audio_capture_client.ReleaseBuffer(frames_available)?;
                }
                WAIT_TIMEOUT => (), // For periodically checking if processing should stop
                _ => return Err(windows::core::Error::from_win32()), // Last error
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_capture_init() {
        let mut audio_capture = AudioCapturer::new(None);
        std::thread::sleep(std::time::Duration::from_millis(500));

        let now = std::time::Instant::now();
        while now.elapsed() < std::time::Duration::from_millis(100) {
            let audio_data = audio_capture.get_audio_data();
            println!("flags: {} qpc: {}", audio_data.flags, audio_data.timestamp);
        }
    }
}

mod audio_format;

use self::audio_format::AudioFormat;
use crate::audio_data::{AudioData, AudioDataDrop, AudioFormatType};
use std::{mem::MaybeUninit, ptr::NonNull};
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
            Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            Threading::{CreateEventA, WaitForSingleObject},
        },
    },
};

pub struct AudioDuplicatorImpl {
    audio_capture_client: IAudioCaptureClient,
    buffer_event: HANDLE,
    audio_format: AudioFormat,
    audio_format_type: AudioFormatType,
    device_id: String,
}

impl AudioDuplicatorImpl {
    pub fn new(device_id: Option<String>) -> Result<Self, windows::core::Error> {
        unsafe {
            // COM usage is in the same process
            let class_context = CLSCTX_INPROC_SERVER;

            let device_enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, class_context)?;

            let (device, device_id) =
                AudioDuplicatorImpl::get_source_device(&device_enumerator, device_id)?;

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
                                audio_format.sampling_rate(),
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
                        AudioFormat::opus_encoder_input_format(audio_format.sampling_rate())?;
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
                device_id,
            })
        }
    }

    /// Get an audio rendering device (i.e, headphones, speakers) from which the audio will be
    /// duplicated from.
    fn get_source_device(
        device_enumerator: &IMMDeviceEnumerator,
        device_id: Option<String>,
    ) -> Result<(IMMDevice, String), windows::core::Error> {
        unsafe {
            let device = match device_id.as_deref() {
                Some(device_id) => device_enumerator.GetDevice(&HSTRING::from(device_id))?,
                None => {
                    let dataflow = eRender;
                    let role = eConsole;
                    let device = device_enumerator.GetDefaultAudioEndpoint(dataflow, role)?;

                    device
                }
            };

            let device_id = device.GetId()?.to_string().unwrap_or_default();

            Ok((device, device_id))
        }
    }

    /// Get the next packet of audio data.
    ///
    /// The audio data can either be 16-bit signed PCM or 32-bit IEEE float depending on the
    /// return value of `audio_format_type`. This function returns `Ok(None)` if the time specified
    /// in `wait_millis` elapses before the next audio data is ready.
    pub fn get_audio_data<'a>(
        &'a self,
        wait_millis: u32,
    ) -> Result<Option<AudioDataWrapper<'a>>, windows::core::Error> {
        unsafe {
            match WaitForSingleObject(self.buffer_event, wait_millis) {
                WAIT_OBJECT_0 => {
                    let mut data = MaybeUninit::uninit();
                    let mut num_frames = MaybeUninit::uninit();
                    let mut flags = MaybeUninit::uninit();
                    let mut timestamp = MaybeUninit::uninit();

                    self.audio_capture_client.GetBuffer(
                        data.as_mut_ptr(),
                        num_frames.as_mut_ptr(),
                        flags.as_mut_ptr(),
                        None,
                        Some(timestamp.as_mut_ptr()),
                    )?;

                    // `GetBuffer` returns a non-null pointer on success
                    let data = NonNull::new_unchecked(data.assume_init());

                    let audio_data = AudioData::new(
                        data,
                        num_frames.assume_init(),
                        flags.assume_init(),
                        timestamp.assume_init(),
                    );

                    Ok(Some(AudioDataWrapper {
                        inner: audio_data,
                        parent: self,
                    }))
                }
                WAIT_TIMEOUT => Ok(None),
                _ => return Err(windows::core::Error::from_win32()), // Last error
            }
        }
    }

    pub fn num_channels(&self) -> u16 {
        self.audio_format.num_channels()
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn audio_format_type(&self) -> AudioFormatType {
        self.audio_format_type
    }
}

impl AudioDataDrop for AudioDuplicatorImpl {
    fn drop_audio_data<'a>(&self, audio_data: &'a AudioData<'a>) {
        unsafe {
            if let Err(e) = self
                .audio_capture_client
                .ReleaseBuffer(audio_data.num_frames)
            {
                tracing::error!("IAudioCaptureClient::ReleaseBuffer error: {}", e);

                #[cfg(test)]
                panic!("IAudioCaptureClient::ReleaseBuffer error: {}", e);
            }
        }
    }
}

/// RAII wrapper for automatically freeing the buffer returned by `get_audio_data`.
pub struct AudioDataWrapper<'a> {
    inner: AudioData<'a>,
    parent: &'a AudioDuplicatorImpl,
}

impl<'a> Drop for AudioDataWrapper<'a> {
    fn drop(&mut self) {
        self.parent.drop_audio_data(&self.inner);
    }
}

impl<'a> std::ops::Deref for AudioDataWrapper<'a> {
    type Target = AudioData<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_capture_init() {
        AudioDuplicatorImpl::new(None).unwrap();
    }
}

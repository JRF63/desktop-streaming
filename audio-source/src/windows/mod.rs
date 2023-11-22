mod audio_format;

use self::audio_format::AudioFormat;
use crate::{
    audio_data::{AudioData, AudioDataDrop, AudioDataWrapper, AudioFormatKind},
    error::AudioSourceError,
};
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
    audio_format_kind: AudioFormatKind,
    device_id: String,
}

impl AudioDuplicatorImpl {
    pub fn new(device_id: Option<String>) -> Result<Self, AudioSourceError> {
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

            let audio_format_kind = match audio_format.audio_format_kind() {
                Some(t) => {
                    let mut audio_format_kind = t;

                    if !audio_format.is_sampling_rate_supported_by_encoder() {
                        audio_format.set_sampling_rate_to_supported();
                        if !audio_format.is_supported_by_device(&audio_client, share_mode)? {
                            stream_flags |= AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
                            audio_format = AudioFormat::opus_encoder_input_format(
                                audio_format.sampling_rate(),
                            )?;
                            audio_format_kind = AudioFormatKind::Pcm;
                        }
                    }
                    audio_format_kind
                }
                None => {
                    // Format is unsupported by the encoder so auto-convert it to a high-quality
                    // PCM stream.
                    stream_flags |= AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                        | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
                    audio_format =
                        AudioFormat::opus_encoder_input_format(audio_format.sampling_rate())?;
                    AudioFormatKind::Pcm
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
                audio_format_kind,
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

    pub fn get_audio_data<'a>(
        &'a self,
        wait_millis: u32,
    ) -> Result<AudioDataWrapper<'a, AudioDuplicatorImpl>, AudioSourceError> {
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

                    Ok(AudioDataWrapper::new(audio_data, self))
                }
                WAIT_TIMEOUT => Err(AudioSourceError::WaitTimeout),
                _ => return Err(windows::core::Error::from_win32().into()), // Last error
            }
        }
    }

    pub fn num_channels(&self) -> u16 {
        self.audio_format.num_channels()
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn audio_format_kind(&self) -> AudioFormatKind {
        self.audio_format_kind
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

impl From<windows::core::Error> for AudioSourceError {
    fn from(value: windows::core::Error) -> Self {
        tracing::error!("{}", value);

        if value.code() == AUDCLNT_E_DEVICE_INVALIDATED {
            AudioSourceError::DeviceInvalidated
        } else {
            AudioSourceError::InternalError
        }
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

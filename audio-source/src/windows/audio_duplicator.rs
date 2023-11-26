use super::audio_format::AudioFormat;
use crate::{
    audio_data::{AudioData, AudioDataDrop, AudioDataWrapper, AudioFormatKind},
    error::Error,
};
use std::{mem::MaybeUninit, ptr::NonNull};
use windows::{
    core::HSTRING,
    Win32::{
        Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Media::Audio::{
            eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
            MMDeviceEnumerator, AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_SHAREMODE,
            AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
            AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
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
    pub fn new(device_id: Option<String>) -> Result<Self, Error> {
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

            let (audio_format, additional_flags) =
                AudioDuplicatorImpl::get_supported_audio_format(&audio_client, share_mode)?;
            stream_flags |= additional_flags;

            // This `unwrap` should not fail
            let audio_format_kind = audio_format.audio_format_kind().unwrap();

            let buffer_duration = AudioDuplicatorImpl::get_minimum_buffer_duration(&audio_client)?;

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
                    device_enumerator.GetDefaultAudioEndpoint(dataflow, role)?
                }
            };

            let device_id = device.GetId()?.to_string().unwrap_or_default();

            Ok((device, device_id))
        }
    }

    fn get_minimum_buffer_duration(
        audio_client: &IAudioClient,
    ) -> Result<i64, windows::core::Error> {
        let mut default_device_period = MaybeUninit::uninit(); // Not going to be used
        let mut min_device_period = MaybeUninit::uninit();

        unsafe {
            audio_client.GetDevicePeriod(
                Some(default_device_period.as_mut_ptr()),
                Some(min_device_period.as_mut_ptr()),
            )?;
            Ok(min_device_period.assume_init())
        }
    }

    fn get_supported_audio_format(
        audio_client: &IAudioClient,
        share_mode: AUDCLNT_SHAREMODE,
    ) -> Result<(AudioFormat, u32), windows::core::Error> {
        let mut mix_audio_format = AudioFormat::get_mix_format(audio_client)?;

        if mix_audio_format.audio_format_kind().is_some() {
            mix_audio_format.fix_sampling_rate_if_unsupported();
            mix_audio_format.fix_num_audio_channels_if_unsupported();

            if mix_audio_format.is_supported_by_device(audio_client, share_mode)? {
                return Ok((mix_audio_format, 0));
            }
        }

        // Mix format is unsupported by the encoder so auto-convert it to a high-quality PCM stream
        let additional_flags =
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        let opus_encoder_input_format =
            AudioFormat::opus_encoder_input_format(mix_audio_format.sampling_rate())?;
        Ok((opus_encoder_input_format, additional_flags))
    }

    pub fn get_audio_data(
        &self,
        timeout_millis: u32,
    ) -> Result<AudioDataWrapper<'_, AudioDuplicatorImpl>, Error> {
        unsafe {
            match WaitForSingleObject(self.buffer_event, timeout_millis) {
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
                WAIT_TIMEOUT => Err(Error::WaitTimeout),
                _ => Err(windows::core::Error::from_win32().into()), // Last error
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

impl From<windows::core::Error> for Error {
    fn from(value: windows::core::Error) -> Self {
        tracing::error!("{}", value);

        if value.code() == AUDCLNT_E_DEVICE_INVALIDATED {
            Error::DeviceInvalidated
        } else {
            Error::InternalError
        }
    }
}

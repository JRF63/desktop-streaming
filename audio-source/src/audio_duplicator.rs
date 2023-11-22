use crate::{
    audio_data::{AudioDataWrapper, AudioFormatKind},
    error::AudioSourceError,
};

#[cfg(windows)]
use crate::windows::AudioDuplicatorImpl;

#[repr(transparent)]
pub struct AudioDuplicator(AudioDuplicatorImpl);

impl AudioDuplicator {
    /// Create a new `AudioDuplicator`. Passing `None` via the `device_id` will produce the default
    /// audio rendering device, otherwise it will use the device specified by the string.
    pub fn new(device_id: Option<String>) -> Result<Self, AudioSourceError> {
        AudioDuplicatorImpl::new(device_id).and_then(|inner| Ok(Self(inner)))
    }

    /// Get the next packet of audio data.
    ///
    /// The audio data can either be 16-bit signed PCM or 32-bit IEEE float depending on the audio
    /// format type. This function returns `Ok(None)` if the time specified in `wait_millis`
    /// elapses before the next audio data is ready.
    pub fn get_audio_data<'a>(
        &'a self,
        wait_millis: u32,
    ) -> Result<AudioDataWrapper<'a, AudioDuplicatorImpl>, AudioSourceError> {
        self.0.get_audio_data(wait_millis)
    }

    /// Returns a `&str` that is used to identify the current audio device.
    pub fn device_id(&self) -> &str {
        self.0.device_id()
    }

    /// Returns the number of audio channels and the audio format type of the `AudioDuplicator`'s
    /// output.
    pub fn audio_format_info(&self) -> (u16, AudioFormatKind) {
        (self.0.num_channels(), self.0.audio_format_kind())
    }
}

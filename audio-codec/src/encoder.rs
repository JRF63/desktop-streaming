use crate::{
    codec_traits::Encodeable,
    error::Error,
    settings::{ApplicationMode, AudioChannels, Bitrate, SampleRate},
    sys,
};
use std::{mem::MaybeUninit, ptr::NonNull};

pub struct AudioEncoder {
    inner: NonNull<sys::OpusEncoder>,
    num_channels: AudioChannels,
}

// SAFETY: `*mut OpusEncoder` is safe to move to other threads
unsafe impl Send for AudioEncoder {}

impl Drop for AudioEncoder {
    fn drop(&mut self) {
        unsafe {
            sys::opus_encoder_destroy(self.as_inner());
        }
    }
}

impl AudioEncoder {
    pub fn new(
        sample_rate: SampleRate,
        num_channels: AudioChannels,
        mode: ApplicationMode,
    ) -> Result<Self, Error> {
        unsafe {
            let mut error = MaybeUninit::uninit();
            let encoder = sys::opus_encoder_create(
                sample_rate as i32,
                num_channels as i32,
                mode as i32,
                error.as_mut_ptr(),
            );

            if let Some(e) = Error::from_raw_error_code(error.assume_init()) {
                return Err(e);
            }

            match NonNull::new(encoder) {
                Some(encoder) => Ok(Self {
                    inner: encoder,
                    num_channels,
                }),
                None => Err(Error::InternalError),
            }
        }
    }

    pub fn set_bitrate(&mut self, bitrate: Bitrate) -> Result<(), Error> {
        unsafe {
            let error =
                sys::opus_encoder_ctl(self.as_inner(), sys::OPUS_SET_BITRATE_REQUEST, bitrate.0);
            match Error::from_raw_error_code(error) {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    }

    pub fn get_bitrate(&mut self) -> Result<Bitrate, Error> {
        unsafe {
            let mut bitrate = MaybeUninit::uninit();
            let error = sys::opus_encoder_ctl(
                self.as_inner(),
                sys::OPUS_GET_BITRATE_REQUEST,
                bitrate.as_mut_ptr(),
            );
            match Error::from_raw_error_code(error) {
                Some(e) => Err(e),
                None => Ok(Bitrate(bitrate.assume_init())),
            }
        }
    }

    pub unsafe fn encode_raw<T>(
        &mut self,
        input: *const T,
        input_num_frames_per_channel: i32,
        output: &mut [u8],
    ) -> Result<i32, Error>
    where
        T: Encodeable,
    {
        unsafe {
            let error = T::encode(
                self.as_inner(),
                input,
                input_num_frames_per_channel,
                output.as_mut_ptr(),
                output.len() as i32,
            );

            if error <= 0 {
                match Error::from_raw_error_code(error) {
                    Some(e) => Err(e),
                    None => Ok(0),
                }
            } else {
                Ok(error)
            }
        }
    }

    pub fn encode<T>(&mut self, input: &[T], output: &mut [u8]) -> Result<i32, Error>
    where
        T: Encodeable,
    {
        unsafe {
            self.encode_raw(
                input.as_ptr(),
                self.num_channels.num_frames_per_channel(input.len() as i32),
                output,
            )
        }
    }

    fn as_inner(&self) -> *mut sys::OpusEncoder {
        self.inner.as_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_encoder_set_bitrate_test() {
        let mut audio_encoder = AudioEncoder::new(
            SampleRate::Hz48000,
            AudioChannels::Stereo,
            ApplicationMode::LowDelay,
        )
        .unwrap();
        audio_encoder
            .set_bitrate(Bitrate::new(256000).unwrap())
            .unwrap();
    }
}

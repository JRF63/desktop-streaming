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

    /// Sets the expected packet loss of the encoder.
    ///
    /// Values of `percentage` outside the valid range of `0..=100` would result in
    /// `Error::BadArg`.
    ///
    /// This function also enables the inband forward error correction if the loss percentage is
    /// greater than zero and disables it otherwise.
    pub fn set_fec_expected_packet_loss(&mut self, percentage: i32) -> Result<(), Error> {
        unsafe {
            let enable_fec = percentage > 0;

            let error = sys::opus_encoder_ctl(
                self.as_inner(),
                sys::OPUS_SET_INBAND_FEC_REQUEST,
                enable_fec as i32,
            );
            match Error::from_raw_error_code(error) {
                Some(e) => Err(e),
                None => Ok(()),
            }?;

            let error = sys::opus_encoder_ctl(
                self.as_inner(),
                sys::OPUS_SET_PACKET_LOSS_PERC_REQUEST,
                percentage,
            );
            match Error::from_raw_error_code(error) {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    }

    /// Configures the encoder's computational complexity.
    /// 
    /// Values of `complexity` outside the valid range of `0..=10` would result in `Error::BadArg`.
    pub fn set_encoder_complexity(&mut self, complexity: i32) -> Result<(), Error> {
        unsafe {
            let error = sys::opus_encoder_ctl(
                self.as_inner(),
                sys::OPUS_SET_COMPLEXITY_REQUEST,
                complexity,
            );
            match Error::from_raw_error_code(error) {
                Some(e) => Err(e),
                None => Ok(()),
            }
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
            SampleRate::Fullband,
            AudioChannels::Stereo,
            ApplicationMode::LowDelay,
        )
        .unwrap();
    
        audio_encoder
            .set_bitrate(Bitrate::new(256000).unwrap())
            .unwrap();
    }

    #[test]
    fn audio_encoder_set_fec_expected_packet_loss() {
        let mut audio_encoder = AudioEncoder::new(
            SampleRate::Fullband,
            AudioChannels::Stereo,
            ApplicationMode::LowDelay,
        )
        .unwrap();

        audio_encoder.set_fec_expected_packet_loss(0).unwrap();
        audio_encoder.set_fec_expected_packet_loss(100).unwrap();
        assert!(audio_encoder.set_fec_expected_packet_loss(-1).is_err());
        assert!(audio_encoder.set_fec_expected_packet_loss(101).is_err());
    }

    #[test]
    fn audio_encoder_set_encoder_complexity() {
        let mut audio_encoder = AudioEncoder::new(
            SampleRate::Fullband,
            AudioChannels::Stereo,
            ApplicationMode::LowDelay,
        )
        .unwrap();

        audio_encoder.set_encoder_complexity(0).unwrap();
        audio_encoder.set_encoder_complexity(10).unwrap();
        assert!(audio_encoder.set_encoder_complexity(-1).is_err());
        assert!(audio_encoder.set_encoder_complexity(11).is_err());
    }
}

use crate::{
    codec_traits::Decodeable,
    error::Error,
    settings::{AudioChannels, SampleRate},
    sys,
};
use std::{mem::MaybeUninit, ptr::NonNull};

pub struct AudioDecoder {
    inner: NonNull<sys::OpusDecoder>,
    num_channels: AudioChannels,
}

// SAFETY: `*mut OpusDecoder` is safe to move to other threads
unsafe impl Send for AudioDecoder {}

impl Drop for AudioDecoder {
    fn drop(&mut self) {
        unsafe {
            sys::opus_decoder_destroy(self.as_inner());
        }
    }
}

impl AudioDecoder {
    pub fn new(sample_rate: SampleRate, num_channels: AudioChannels) -> Result<Self, Error> {
        unsafe {
            let mut error = MaybeUninit::uninit();
            let decoder = sys::opus_decoder_create(
                sample_rate as i32,
                num_channels as i32,
                error.as_mut_ptr(),
            );

            if let Some(e) = Error::from_raw_error_code(error.assume_init()) {
                return Err(e);
            }

            match NonNull::new(decoder) {
                Some(decoder) => Ok(Self {
                    inner: decoder,
                    num_channels,
                }),
                None => Err(Error::InternalError),
            }
        }
    }

    pub unsafe fn decode_raw<T>(
        &mut self,
        input: &[u8],
        output: *mut T,
        output_num_frames_per_channel: i32,
        decode_fec: bool,
    ) -> Result<i32, Error>
    where
        T: Decodeable,
    {
        unsafe {
            let error = T::decode(
                self.as_inner(),
                input.as_ptr(),
                input.len() as i32,
                output,
                output_num_frames_per_channel,
                decode_fec as i32,
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

    pub fn decode<T>(
        &mut self,
        input: &[u8],
        output: &mut [T],
        decode_fec: bool,
    ) -> Result<i32, Error>
    where
        T: Decodeable,
    {
        unsafe {
            self.decode_raw(
                input,
                output.as_mut_ptr(),
                self.num_channels
                    .num_frames_per_channel(output.len() as i32),
                decode_fec,
            )
        }
    }

    fn as_inner(&self) -> *mut sys::OpusDecoder {
        self.inner.as_ptr()
    }
}

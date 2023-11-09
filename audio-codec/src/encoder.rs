use crate::sys;
use std::{mem::MaybeUninit, ptr::NonNull};

pub struct AudioEncoder {
    encoder: NonNull<sys::OpusEncoder>,
}

// SAFETY: `*mut OpusEncoder` is safe to move to other threads
unsafe impl Send for AudioEncoder {}

impl Drop for AudioEncoder {
    fn drop(&mut self) {
        unsafe {
            sys::opus_encoder_destroy(self.encoder.as_ptr());
        }
    }
}

impl AudioEncoder {
    pub fn new(
        sample_rate: SampleRate,
        channels: AudioChannel,
        mode: ApplicationMode,
    ) -> Result<Self, Error> {
        unsafe {
            let mut error = MaybeUninit::uninit();
            let encoder = sys::opus_encoder_create(
                sample_rate as i32,
                channels as i32,
                mode as i32,
                error.as_mut_ptr(),
            );

            if let Some(e) = Error::try_from_raw_errorcode(error.assume_init()) {
                return Err(e);
            }

            match NonNull::new(encoder) {
                Some(encoder) => Ok(Self { encoder }),
                None => Err(Error::InternalError),
            }
        }
    }

    pub fn set_bitrate(&mut self, bitrate: Bitrate) -> Result<(), Error> {
        unsafe {
            let ret = sys::opus_encoder_ctl(
                self.encoder.as_ptr(),
                sys::OPUS_SET_BITRATE_REQUEST,
                bitrate.0,
            );
            match Error::try_from_raw_errorcode(ret) {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[repr(i32)]
pub enum Error {
    #[error("One or more invalid/out of range arguments")]
    BadArg = sys::OPUS_BAD_ARG,
    #[error("Not enough bytes allocated in the buffer")]
    BufferTooSmall = sys::OPUS_BUFFER_TOO_SMALL,
    #[error("An internal error was detected")]
    InternalError = sys::OPUS_INTERNAL_ERROR,
    #[error("The compressed data passed is corrupted")]
    InvalidPacket = sys::OPUS_INVALID_PACKET,
    #[error("Invalid/unsupported request number")]
    Unimplemented = sys::OPUS_UNIMPLEMENTED,
    #[error("An encoder or decoder structure is invalid or already freed")]
    InvalidState = sys::OPUS_INVALID_STATE,
    #[error("Memory allocation has failed")]
    AllocFail = sys::OPUS_ALLOC_FAIL,
}

impl Error {
    unsafe fn try_from_raw_errorcode(errorcode: i32) -> Option<Error> {
        match errorcode {
            sys::OPUS_OK => None,
            sys::OPUS_BAD_ARG => Some(Error::BadArg),
            sys::OPUS_BUFFER_TOO_SMALL => Some(Error::BufferTooSmall),
            sys::OPUS_INTERNAL_ERROR => Some(Error::InternalError),
            sys::OPUS_INVALID_PACKET => Some(Error::InvalidPacket),
            sys::OPUS_UNIMPLEMENTED => Some(Error::Unimplemented),
            sys::OPUS_INVALID_STATE => Some(Error::InvalidState),
            sys::OPUS_ALLOC_FAIL => Some(Error::AllocFail),
            _ => std::hint::unreachable_unchecked(),
        }
    }
}

/// Sampling rate (Hz).
#[repr(i32)]
pub enum SampleRate {
    Hz8000 = 8000,
    Hz12000 = 12000,
    Hz16000 = 16000,
    Hz24000 = 24000,
    Hz48000 = 48000,
}

#[repr(i32)]
pub enum AudioChannel {
    Mono = 1,
    Stereo = 2,
}

/// Audio encoder coding modes.
#[repr(i32)]
pub enum ApplicationMode {
    /// Gives best quality at a given bitrate for voice signals.
    Voip = sys::OPUS_APPLICATION_VOIP,
    /// Gives best quality at a given bitrate for most non-voice signals like music.
    Audio = sys::OPUS_APPLICATION_AUDIO,
    /// Configures low-delay mode that disables the speech-optimized mode in exchange for slightly
    /// reduced delay.
    LowDelay = sys::OPUS_APPLICATION_RESTRICTED_LOWDELAY,
}

pub struct Bitrate(i32);

impl Bitrate {
    /// Let the encoder choose the bitrate.
    pub const AUTO: Bitrate = Bitrate(sys::OPUS_AUTO);
    /// Makes the encoder use as much bitrate as possible.
    pub const MAX: Bitrate = Bitrate(sys::OPUS_BITRATE_MAX);

    /// Create a new `Bitrate`. Valid bitrate range is from 500 to 512000 bits per second.
    pub fn new(bitrate: i32) -> Option<Self> {
        match bitrate {
            500..=512000 => Some(Self(bitrate)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_encoder_init_test() {
        AudioEncoder::new(
            SampleRate::Hz48000,
            AudioChannel::Stereo,
            ApplicationMode::LowDelay,
        )
        .unwrap();
    }

    #[test]
    fn audio_encoder_set_bitrate_test() {
        let mut audio_encoder = AudioEncoder::new(
            SampleRate::Hz48000,
            AudioChannel::Stereo,
            ApplicationMode::LowDelay,
        )
        .unwrap();
        audio_encoder
            .set_bitrate(Bitrate::new(256000).unwrap())
            .unwrap();
    }
}

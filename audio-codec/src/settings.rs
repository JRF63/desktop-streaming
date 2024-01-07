use crate::sys;

/// Sampling rate (Hz).
///
// Sample rate naming: https://developer.mozilla.org/en-US/docs/Web/Media/Formats/Audio_codecs#opus
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum SampleRate {
    /// 8 kHz
    Narrowband = 8000,
    /// 12 kHz
    MediumBand = 12000,
    /// 16 kHz
    Wideband = 16000,
    /// 24 kHz
    SuperWideband = 24000,
    /// 48 kHz
    Fullband = 48000,
}

/// Number of audio channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum AudioChannels {
    Mono = 1,
    Stereo = 2,
}

impl AudioChannels {
    /// Calculate the number of frames per channel from the total number of frames.
    #[inline]
    pub const fn num_frames_per_channel(self, num_frames: i32) -> i32 {
        // Produces better assembly than just plain division
        num_frames >> (self as i32 - 1)
    }
}

/// Audio encoder coding modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// Bits per second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Bitrate(pub(crate) i32);

impl Bitrate {
    /// Let the encoder choose the bitrate.
    pub const AUTO: Bitrate = Bitrate(sys::OPUS_AUTO);
    /// Makes the encoder use as much bitrate as possible.
    pub const MAX: Bitrate = Bitrate(sys::OPUS_BITRATE_MAX);

    /// Create a new `Bitrate`. Valid bitrate range is from 500 to 512000 bits per second.
    pub const fn new(bitrate: i32) -> Option<Self> {
        const MIN_BITRATE: i32 = 500;
        const MAX_BITRATE: i32 = 512000;

        match bitrate {
            MIN_BITRATE..=MAX_BITRATE => Some(Self(bitrate)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_channels_num_frames_per_channel() {
        let values = [240, 480, 960, 1920, 3840, 5760];
        for num_frames in values {
            let mono = AudioChannels::Mono;
            let stereo = AudioChannels::Stereo;

            assert_eq!(mono.num_frames_per_channel(num_frames), num_frames);
            assert_eq!(stereo.num_frames_per_channel(num_frames), num_frames / 2);
        }
    }
}

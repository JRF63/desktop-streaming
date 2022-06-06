mod encoder;
mod error;
mod guids;
mod util;
pub(crate) mod os;

use guids::*;

#[macro_export]
macro_rules! nvenc_function {
    ($fn:expr, $($arg:expr),*) => {
        let status = ($fn.unwrap_or_else(|| std::hint::unreachable_unchecked()))($($arg,)*);
        if let Some(error) = crate::error::NvEncError::new(status) {
            return Err(error.into());
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Codec {
    H264,
    Hevc,
}

impl Into<nvenc_sys::GUID> for Codec {
    fn into(self) -> nvenc_sys::GUID {
        match self {
            Codec::H264 => NV_ENC_CODEC_H264_GUID,
            Codec::Hevc => NV_ENC_CODEC_HEVC_GUID,
        }
    }
}

impl From<nvenc_sys::GUID> for Codec {
    fn from(guid: nvenc_sys::GUID) -> Self {
        if guid == NV_ENC_CODEC_H264_GUID {
            Codec::H264
        } else if guid == NV_ENC_CODEC_HEVC_GUID {
            Codec::Hevc
        } else {
            panic!("Invalid codec guid.")
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum CodecProfile {
    Autoselect,
    H264Baseline,
    H264Main,
    H264High,
    H264High444,
    H264Stereo,
    H264ProgressiveHigh,
    H264ConstrainedHigh,
    HevcMain,
    HevcMain10,
    HevcFrext,
}

impl Into<nvenc_sys::GUID> for CodecProfile {
    fn into(self) -> nvenc_sys::GUID {
        match self {
            CodecProfile::Autoselect => NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID,
            CodecProfile::H264Baseline => NV_ENC_H264_PROFILE_BASELINE_GUID,
            CodecProfile::H264Main => NV_ENC_H264_PROFILE_MAIN_GUID,
            CodecProfile::H264High => NV_ENC_H264_PROFILE_HIGH_GUID,
            CodecProfile::H264High444 => NV_ENC_H264_PROFILE_HIGH_444_GUID,
            CodecProfile::H264Stereo => NV_ENC_H264_PROFILE_STEREO_GUID,
            CodecProfile::H264ProgressiveHigh => NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID,
            CodecProfile::H264ConstrainedHigh => NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID,
            CodecProfile::HevcMain => NV_ENC_HEVC_PROFILE_MAIN_GUID,
            CodecProfile::HevcMain10 => NV_ENC_HEVC_PROFILE_MAIN10_GUID,
            CodecProfile::HevcFrext => NV_ENC_HEVC_PROFILE_FREXT_GUID,
        }
    }
}

impl From<nvenc_sys::GUID> for CodecProfile {
    fn from(guid: nvenc_sys::GUID) -> Self {
        if guid == NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID {
            CodecProfile::Autoselect
        } else if guid == NV_ENC_H264_PROFILE_BASELINE_GUID {
            CodecProfile::H264Baseline
        } else if guid == NV_ENC_H264_PROFILE_MAIN_GUID {
            CodecProfile::H264Main
        } else if guid == NV_ENC_H264_PROFILE_HIGH_GUID {
            CodecProfile::H264High
        } else if guid == NV_ENC_H264_PROFILE_HIGH_444_GUID {
            CodecProfile::H264High444
        } else if guid == NV_ENC_H264_PROFILE_STEREO_GUID {
            CodecProfile::H264Stereo
        } else if guid == NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID {
            CodecProfile::H264ProgressiveHigh
        } else if guid == NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID {
            CodecProfile::H264ConstrainedHigh
        } else if guid == NV_ENC_HEVC_PROFILE_MAIN_GUID {
            CodecProfile::HevcMain
        } else if guid == NV_ENC_HEVC_PROFILE_MAIN10_GUID {
            CodecProfile::HevcMain10
        } else if guid == NV_ENC_HEVC_PROFILE_FREXT_GUID {
            CodecProfile::HevcFrext
        } else {
            panic!("Invalid codec profile guid.")
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum EncoderPreset {
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
}

impl Into<nvenc_sys::GUID> for EncoderPreset {
    fn into(self) -> nvenc_sys::GUID {
        match self {
            EncoderPreset::P1 => NV_ENC_PRESET_P1_GUID,
            EncoderPreset::P2 => NV_ENC_PRESET_P2_GUID,
            EncoderPreset::P3 => NV_ENC_PRESET_P3_GUID,
            EncoderPreset::P4 => NV_ENC_PRESET_P4_GUID,
            EncoderPreset::P5 => NV_ENC_PRESET_P5_GUID,
            EncoderPreset::P6 => NV_ENC_PRESET_P6_GUID,
            EncoderPreset::P7 => NV_ENC_PRESET_P7_GUID,
        }
    }
}

impl From<nvenc_sys::GUID> for EncoderPreset {
    fn from(guid: nvenc_sys::GUID) -> Self {
        if guid == NV_ENC_PRESET_P1_GUID {
            EncoderPreset::P1
        } else if guid == NV_ENC_PRESET_P2_GUID {
            EncoderPreset::P2
        } else if guid == NV_ENC_PRESET_P3_GUID {
            EncoderPreset::P3
        } else if guid == NV_ENC_PRESET_P4_GUID {
            EncoderPreset::P4
        } else if guid == NV_ENC_PRESET_P5_GUID {
            EncoderPreset::P5
        } else if guid == NV_ENC_PRESET_P6_GUID {
            EncoderPreset::P6
        } else if guid == NV_ENC_PRESET_P7_GUID {
            EncoderPreset::P7
        } else {
            panic!("Invalid encoder preset.")
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum TuningInfo {
    Undefined,
    HighQuality,
    LowLatency,
    UltraLowLatency,
    Lossless,
}

impl Into<nvenc_sys::NV_ENC_TUNING_INFO> for TuningInfo {
    fn into(self) -> nvenc_sys::NV_ENC_TUNING_INFO {
        match self {
            TuningInfo::Undefined => nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_UNDEFINED,
            TuningInfo::HighQuality => nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_HIGH_QUALITY,
            TuningInfo::LowLatency => nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_LOW_LATENCY,
            TuningInfo::UltraLowLatency => nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY,
            TuningInfo::Lossless => nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_LOSSLESS,
        }
    }
}

impl From<nvenc_sys::NV_ENC_TUNING_INFO> for TuningInfo {
    fn from(tuning_info: nvenc_sys::NV_ENC_TUNING_INFO) -> Self {
        match tuning_info {
            nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_UNDEFINED => TuningInfo::Undefined,
            nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_HIGH_QUALITY => TuningInfo::HighQuality,
            nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_LOW_LATENCY => TuningInfo::LowLatency,
            nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY => TuningInfo::UltraLowLatency,
            nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_LOSSLESS => TuningInfo::Lossless,
            _ => panic!("Invalid tuning info."),
        }
    }
}

pub use encoder::create_encoder;
use crate::{nvenc_function, guids, RawEncoder, Result};
use std::mem::MaybeUninit;

#[derive(Debug, Copy, Clone)]
pub enum Codec {
    H264,
    Hevc,
}

impl Codec {
    pub(crate) fn to_guid(self) -> nvenc_sys::GUID {
        match self {
            Codec::H264 => guids::NV_ENC_CODEC_H264_GUID,
            Codec::Hevc => guids::NV_ENC_CODEC_HEVC_GUID,
        }
    }

    pub(crate) fn from_guid(guid: nvenc_sys::GUID) -> Self {
        if guid == guids::NV_ENC_CODEC_H264_GUID {
            Codec::H264
        } else if guid == guids::NV_ENC_CODEC_HEVC_GUID {
            Codec::Hevc
        } else {
            panic!("Invalid codec guid")
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

impl CodecProfile {
    pub(crate) fn to_guid(self) -> nvenc_sys::GUID {
        match self {
            CodecProfile::Autoselect => guids::NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID,
            CodecProfile::H264Baseline => guids::NV_ENC_H264_PROFILE_BASELINE_GUID,
            CodecProfile::H264Main => guids::NV_ENC_H264_PROFILE_MAIN_GUID,
            CodecProfile::H264High => guids::NV_ENC_H264_PROFILE_HIGH_GUID,
            CodecProfile::H264High444 => guids::NV_ENC_H264_PROFILE_HIGH_444_GUID,
            CodecProfile::H264Stereo => guids::NV_ENC_H264_PROFILE_STEREO_GUID,
            CodecProfile::H264ProgressiveHigh => guids::NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID,
            CodecProfile::H264ConstrainedHigh => guids::NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID,
            CodecProfile::HevcMain => guids::NV_ENC_HEVC_PROFILE_MAIN_GUID,
            CodecProfile::HevcMain10 => guids::NV_ENC_HEVC_PROFILE_MAIN10_GUID,
            CodecProfile::HevcFrext => guids::NV_ENC_HEVC_PROFILE_FREXT_GUID,
        }
    }

    pub(crate) fn from_guid(guid: nvenc_sys::GUID) -> Self {
        if guid == guids::NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID {
            CodecProfile::Autoselect
        } else if guid == guids::NV_ENC_H264_PROFILE_BASELINE_GUID {
            CodecProfile::H264Baseline
        } else if guid == guids::NV_ENC_H264_PROFILE_MAIN_GUID {
            CodecProfile::H264Main
        } else if guid == guids::NV_ENC_H264_PROFILE_HIGH_GUID {
            CodecProfile::H264High
        } else if guid == guids::NV_ENC_H264_PROFILE_HIGH_444_GUID {
            CodecProfile::H264High444
        } else if guid == guids::NV_ENC_H264_PROFILE_STEREO_GUID {
            CodecProfile::H264Stereo
        } else if guid == guids::NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID {
            CodecProfile::H264ProgressiveHigh
        } else if guid == guids::NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID {
            CodecProfile::H264ConstrainedHigh
        } else if guid == guids::NV_ENC_HEVC_PROFILE_MAIN_GUID {
            CodecProfile::HevcMain
        } else if guid == guids::NV_ENC_HEVC_PROFILE_MAIN10_GUID {
            CodecProfile::HevcMain10
        } else if guid == guids::NV_ENC_HEVC_PROFILE_FREXT_GUID {
            CodecProfile::HevcFrext
        } else {
            panic!("Invalid codec profile guid")
        }
    }
}

impl<const BUF_SIZE: usize> RawEncoder<BUF_SIZE> {
    pub fn codec_profiles(&self, codec: Codec) -> Result<Vec<CodecProfile>> {
        let encode_guid = codec.to_guid();
        let profile_guids = self.encode_profile_guids(encode_guid)?;
        let encode_profiles = profile_guids
            .iter()
            .map(|guid| CodecProfile::from_guid(*guid))
            .collect();
        Ok(encode_profiles)
    }

    pub fn codecs(&self) -> Result<Vec<Codec>> {
        let encode_guids = self.encode_guids()?;
        let codecs = encode_guids
            .iter()
            .map(|guid| Codec::from_guid(*guid))
            .collect();
        Ok(codecs)
    }

    fn encode_guid_count(&self) -> Result<u32> {
        let mut encode_guid_count = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeGUIDCount,
                self.raw_encoder.as_ptr(),
                encode_guid_count.as_mut_ptr()
            );
            Ok(encode_guid_count.assume_init())
        }
    }

    fn encode_profile_guid_count(&self, encode_guid: nvenc_sys::GUID) -> Result<u32> {
        let mut encode_profile_guid_count = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeProfileGUIDCount,
                self.raw_encoder.as_ptr(),
                encode_guid,
                encode_profile_guid_count.as_mut_ptr()
            );
            Ok(encode_profile_guid_count.assume_init())
        }
    }

    fn encode_profile_guids(
        &self,
        encode_guid: nvenc_sys::GUID,
    ) -> Result<Vec<nvenc_sys::GUID>> {
        let encode_profile_guid_count = self.encode_profile_guid_count(encode_guid)?;
        let mut profile_guids = Vec::with_capacity(encode_profile_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeProfileGUIDs,
                self.raw_encoder.as_ptr(),
                encode_guid,
                profile_guids.as_mut_ptr(),
                encode_profile_guid_count,
                num_entries.as_mut_ptr()
            );
            profile_guids.set_len(num_entries.assume_init() as usize);
        }
        Ok(profile_guids)
    }

    fn encode_guids(&self) -> Result<Vec<nvenc_sys::GUID>> {
        let encode_guid_count = self.encode_guid_count()?;
        let mut encode_guids = Vec::with_capacity(encode_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeGUIDs,
                self.raw_encoder.as_ptr(),
                encode_guids.as_mut_ptr(),
                encode_guid_count,
                num_entries.as_mut_ptr()
            );
            encode_guids.set_len(num_entries.assume_init() as usize);
        }
        Ok(encode_guids)
    }
}

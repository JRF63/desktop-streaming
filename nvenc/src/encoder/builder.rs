use super::raw_encoder::RawEncoder;
use crate::{Codec, CodecProfile, EncodePreset, Result};
use std::mem::MaybeUninit;

pub struct EncoderBuilder {
    raw_encoder: RawEncoder,
}

impl EncoderBuilder {
    /// List all supported codecs (H.264, HEVC, etc.).
    pub fn codecs(&self) -> Result<Vec<Codec>> {
        let codec_guid_count = unsafe {
            let mut tmp = MaybeUninit::uninit();
            self.raw_encoder.get_encode_guid_count(tmp.as_mut_ptr())?;
            tmp.assume_init()
        };

        let mut codec_guids = Vec::with_capacity(codec_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            self.raw_encoder.get_encode_guids(
                codec_guids.as_mut_ptr(),
                codec_guid_count,
                num_entries.as_mut_ptr(),
            )?;
            codec_guids.set_len(num_entries.assume_init() as usize);
        }

        let codecs = codec_guids.iter().map(|guid| (*guid).into()).collect();
        Ok(codecs)
    }

    /// Lists the profiles available for a codec.
    pub fn codec_profiles(&self, codec: Codec) -> Result<Vec<CodecProfile>> {
        let codec = codec.into();
        let profile_guid_count = unsafe {
            let mut tmp = MaybeUninit::uninit();
            self.raw_encoder
                .get_encode_profile_guid_count(codec, tmp.as_mut_ptr())?;
            tmp.assume_init()
        };

        let mut profile_guids = Vec::with_capacity(profile_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            self.raw_encoder.get_encode_profile_guids(
                codec,
                profile_guids.as_mut_ptr(),
                profile_guid_count,
                num_entries.as_mut_ptr(),
            )?;
            profile_guids.set_len(num_entries.assume_init() as usize);
        }

        let codec_profiles = profile_guids.iter().map(|guid| (*guid).into()).collect();
        Ok(codec_profiles)
    }

    /// Lists the encode presets available for a codec.
    pub fn encode_presets(&self, codec: Codec) -> Result<Vec<EncodePreset>> {
        let codec = codec.into();
        let preset_guid_count = unsafe {
            let mut tmp = MaybeUninit::uninit();
            self.raw_encoder
                .get_encode_preset_count(codec, tmp.as_mut_ptr())?;
            tmp.assume_init()
        };

        let mut preset_guids = Vec::with_capacity(preset_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            self.raw_encoder.get_encode_preset_guids(
                codec,
                preset_guids.as_mut_ptr(),
                preset_guid_count,
                num_entries.as_mut_ptr(),
            )?;
            preset_guids.set_len(num_entries.assume_init() as usize);
        }

        let presets = preset_guids.iter().map(|guid| (*guid).into()).collect();
        Ok(presets)
    }

    /// Lists the supported input formats for a given codec.
    pub fn supported_input_formats(
        &self,
        codec: Codec,
    ) -> Result<Vec<crate::sys::NV_ENC_BUFFER_FORMAT>> {
        let codec = codec.into();
        let mut tmp = MaybeUninit::uninit();
        let input_format_count = unsafe {
            self.raw_encoder
                .get_input_format_count(codec, tmp.as_mut_ptr())?;
            tmp.assume_init()
        };

        let mut input_formats = Vec::with_capacity(input_format_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            self.raw_encoder.get_input_formats(
                codec,
                input_formats.as_mut_ptr(),
                input_format_count,
                num_entries.as_mut_ptr(),
            )?;
            input_formats.set_len(num_entries.assume_init() as usize);
        }
        Ok(input_formats)
    }
}

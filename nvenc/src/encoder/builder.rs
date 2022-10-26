use super::{device::DeviceImplTrait, library::Library, raw_encoder_v2::RawEncoder};
use crate::{Codec, CodecProfile, EncodePreset, NvEncError, Result};
use std::mem::MaybeUninit;

/// Checks if the user's NvEncAPI version is supported.
fn is_version_supported(version: u32) -> bool {
    // TODO: Change this logic once older versions (9.0 to 10.0) are supported
    let major_version = version >> 4;
    let minor_version = version & 0b1111;
    if major_version >= crate::sys::NVENCAPI_MAJOR_VERSION
        && minor_version >= crate::sys::NVENCAPI_MINOR_VERSION
    {
        true
    } else {
        false
    }
}

pub struct EncoderBuilder<D>
where
    D: DeviceImplTrait,
{
    device: D,
    raw_encoder: RawEncoder,
    max_supported_version: u32,
    codec: Option<Codec>,
    codec_profile: Option<CodecProfile>,
    encode_preset: Option<EncodePreset>,
}

impl<D> EncoderBuilder<D>
where
    D: DeviceImplTrait,
{
    pub fn new(device: D) -> Result<Self> {
        let library = Library::load()?;

        let max_supported_version = library.get_max_supported_version()?;

        if !is_version_supported(max_supported_version) {
            return Err(NvEncError::UnsupportedVersion);
        }

        let raw_encoder = RawEncoder::new(&device, library)?;

        Ok(EncoderBuilder {
            device,
            raw_encoder,
            max_supported_version,
            codec: None,
            codec_profile: None,
            encode_preset: None,
        })
    }

    pub fn with_codec(&mut self, codec: Codec) -> Result<&mut Self> {
        if self.supported_codecs()?.contains(&codec) {
            self.codec = Some(codec);
            Ok(self)
        } else {
            Err(NvEncError::UnsupportedCodec)
        }
    }

    pub fn with_codec_profile(&mut self, codec_profile: CodecProfile) -> Result<&mut Self> {
        if self
            .supported_codec_profiles(self.codec.ok_or(NvEncError::CodecNotSet)?)?
            .contains(&codec_profile)
        {
            self.codec_profile = Some(codec_profile);
            Ok(self)
        } else {
            Err(NvEncError::CodecProfileNotSupported)
        }
    }

    pub fn with_encode_preset(&mut self, encode_preset: EncodePreset) -> Result<&mut Self> {
        if self
            .supported_encode_presets(self.codec.ok_or(NvEncError::CodecNotSet)?)?
            .contains(&encode_preset)
        {
            self.encode_preset = Some(encode_preset);
            Ok(self)
        } else {
            Err(NvEncError::CodecProfileNotSupported)
        }
    }

    pub fn build(self) {
        todo!()
    }

    /// List all supported codecs (H.264, HEVC, etc.).
    pub fn supported_codecs(&self) -> Result<Vec<Codec>> {
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
    pub fn supported_codec_profiles(&self, codec: Codec) -> Result<Vec<CodecProfile>> {
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
    pub fn supported_encode_presets(&self, codec: Codec) -> Result<Vec<EncodePreset>> {
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

use super::{raw_encoder::RawEncoder, texture::IntoNvEncBufferFormat};
use crate::{Codec, CodecProfile, EncodePreset, Result, TuningInfo};
use std::{mem::MaybeUninit, ptr::addr_of_mut};

#[repr(transparent)]
pub struct EncodeParams(crate::sys::NV_ENC_RECONFIGURE_PARAMS);

impl Drop for EncodeParams {
    fn drop(&mut self) {
        let ptr = self.0.reInitEncodeParams.encodeConfig;
        debug_assert!(
            !ptr.is_null(),
            "reInitEncodeParams.encodeConfig should not be null"
        );

        // SAFETY: The pointer was allocated by `Box::new` inside `get_codec_config_for_preset`
        let boxed = unsafe { Box::from_raw(ptr) };
        std::mem::drop(boxed);
    }
}

impl EncodeParams {
    pub fn new<T: IntoNvEncBufferFormat>(
        raw_encoder: &RawEncoder,
        width: u32,
        height: u32,
        display_aspect_ratio: Option<(u32, u32)>,
        refresh_rate_ratio: (u32, u32),
        texture_format: &T,
        codec: Codec,
        profile: CodecProfile,
        preset: EncodePreset,
        tuning_info: TuningInfo,
        extra_options: &ExtraOptions,
    ) -> Result<Self> {
        let mut reconfig_params: crate::sys::NV_ENC_RECONFIGURE_PARAMS =
            unsafe { MaybeUninit::zeroed().assume_init() };
        reconfig_params.version = crate::sys::NV_ENC_RECONFIGURE_PARAMS_VER;

        let init_params = &mut reconfig_params.reInitEncodeParams;
        init_params.version = crate::sys::NV_ENC_INITIALIZE_PARAMS_VER;
        init_params.encodeGUID = codec.into();
        init_params.presetGUID = preset.into();
        init_params.encodeWidth = width;
        init_params.encodeHeight = height;

        if let Some((dar_width, dar_height)) = display_aspect_ratio {
            init_params.darWidth = dar_width;
            init_params.darHeight = dar_height;
        } else {
            let gcd = crate::util::gcd(width, height);
            init_params.darWidth = width / gcd;
            init_params.darHeight = height / gcd;
        }

        init_params.frameRateNum = refresh_rate_ratio.0;
        init_params.frameRateDen = refresh_rate_ratio.1;
        init_params.enablePTD = 1;
        init_params.tuningInfo = tuning_info.into();

        #[cfg(windows)]
        {
            // The latency is orders of magnitude higher if synchronous encoding mode is used on
            // Windows based both on testing and according to the docs:
            // https://docs.nvidia.com/video-technologies/video-codec-sdk/nvenc-video-encoder-api-prog-guide/
            init_params.enableEncodeAsync = 1;
            init_params.set_enableOutputInVidmem(0);

            // TODO: Need to pass-in the device type and if directx12, bufferFormat must be set
            // init_params.bufferFormat = ...;
        }

        let codec_config = build_encode_config(
            raw_encoder,
            texture_format,
            codec,
            profile,
            preset,
            tuning_info,
            extra_options,
        )?;

        init_params.encodeConfig = Box::into_raw(codec_config);

        Ok(EncodeParams(reconfig_params))
    }

    pub fn initialize_encoder(&mut self, raw_encoder: &RawEncoder) -> Result<()> {
        unsafe { raw_encoder.initialize_encoder(&mut self.0.reInitEncodeParams) }
    }

    pub fn set_average_bitrate(&mut self, raw_encoder: &RawEncoder, bitrate: u32) -> Result<()> {
        let ptr = self.0.reInitEncodeParams.encodeConfig;
        debug_assert!(
            !ptr.is_null(),
            "reInitEncodeParams.encodeConfig should not be null"
        );

        let encoder_config = unsafe { &mut *ptr };
        encoder_config.rcParams.averageBitRate = bitrate;

        unsafe { raw_encoder.reconfigure_encoder(&mut self.0) }
    }

    pub fn encode_width(&self) -> u32 {
        self.0.reInitEncodeParams.encodeWidth
    }

    pub fn encode_height(&self) -> u32 {
        self.0.reInitEncodeParams.encodeHeight
    }
}

fn build_encode_config<T: IntoNvEncBufferFormat>(
    raw_encoder: &RawEncoder,
    texture_format: &T,
    codec: Codec,
    profile: CodecProfile,
    preset: EncodePreset,
    tuning_info: TuningInfo,
    extra_options: &ExtraOptions,
) -> Result<Box<crate::sys::NV_ENC_CONFIG>> {
    let mut encode_config = unsafe {
        let mut tmp: MaybeUninit<crate::sys::NV_ENC_PRESET_CONFIG> = MaybeUninit::zeroed();

        let ptr = tmp.as_mut_ptr();

        addr_of_mut!((*ptr).version).write(crate::sys::NV_ENC_PRESET_CONFIG_VER);
        addr_of_mut!((*ptr).presetCfg.version).write(crate::sys::NV_ENC_CONFIG_VER);
        raw_encoder.get_encode_preset_config_ex(
            codec.into(),
            preset.into(),
            tuning_info.into(),
            ptr,
        )?;
        tmp.assume_init().presetCfg
    };

    // Need to set the profile after `NvEncGetEncodePresetConfigEx` because it will get wiped
    // otherwise. A zeroed GUID is a valid value for the profileGUID in which case the encoder
    // autoselects a profile.
    encode_config.profileGUID = profile.into();

    extra_options.modify_encode_config(&mut encode_config);

    let codec_config = &mut encode_config.encodeCodecConfig;

    match codec {
        Codec::H264 => {
            let h264_config = unsafe { &mut codec_config.h264Config.as_mut() };

            extra_options.modify_h264_encode_config(h264_config);

            let nvenc_format = texture_format.into_nvenc_buffer_format();
            h264_config.chromaFormatIDC = chroma_format_idc(&nvenc_format);

            // https://docs.nvidia.com/video-technologies/video-codec-sdk/nvenc-video-encoder-api-prog-guide/
            // Settings for optimal performance when using
            // `IDXGIOutputDuplication::AcquireNextFrame`
            #[cfg(windows)]
            {
                h264_config.set_enableFillerDataInsertion(0);
                h264_config.set_outputBufferingPeriodSEI(0);
                h264_config.set_outputPictureTimingSEI(0);
                h264_config.set_outputAUD(0);
                h264_config.set_outputFramePackingSEI(0);
                h264_config.set_outputRecoveryPointSEI(0);
                h264_config.set_enableScalabilityInfoSEI(0);
                h264_config.set_disableSVCPrefixNalu(1);
            }
        }
        Codec::Hevc => {
            let hevc_config = unsafe { &mut codec_config.hevcConfig.as_mut() };

            extra_options.modify_hevc_encode_config(hevc_config);

            let nvenc_format = texture_format.into_nvenc_buffer_format();
            hevc_config.set_chromaFormatIDC(chroma_format_idc(&nvenc_format));
            hevc_config.set_pixelBitDepthMinus8(pixel_bit_depth_minus_8(&nvenc_format));

            // Same settings needed for `AcquireNextFrame`
            #[cfg(windows)]
            {
                hevc_config.set_enableFillerDataInsertion(0);
                hevc_config.set_outputBufferingPeriodSEI(0);
                hevc_config.set_outputPictureTimingSEI(0);
                hevc_config.set_outputAUD(0);
                hevc_config.set_enableAlphaLayerEncoding(0);
            }
        }
    }

    Ok(Box::new(encode_config))
}

pub struct ExtraOptions {
    inband_csd_disabled: bool,
    csd_should_repeat: bool,
    spatial_sq_enabled: bool,
}

impl Default for ExtraOptions {
    fn default() -> Self {
        Self {
            inband_csd_disabled: false,
            csd_should_repeat: false,
            spatial_sq_enabled: false,
        }
    }
}

impl ExtraOptions {
    pub(crate) fn disable_inband_csd(&mut self) {
        self.inband_csd_disabled = true;
    }

    pub(crate) fn repeat_csd(&mut self) {
        self.csd_should_repeat = true;
    }

    pub(crate) fn enable_spatial_aq(&mut self) {
        self.spatial_sq_enabled = true;
    }

    fn modify_encode_config(&self, config: &mut crate::sys::NV_ENC_CONFIG) {
        if self.spatial_sq_enabled {
            config.rcParams.set_enableAQ(1);
        }
    }

    fn modify_h264_encode_config(&self, h264_config: &mut crate::sys::NV_ENC_CONFIG_H264) {
        if self.inband_csd_disabled {
            h264_config.set_disableSPSPPS(1);
        }
        if self.csd_should_repeat {
            h264_config.set_repeatSPSPPS(1);
        }
    }

    fn modify_hevc_encode_config(&self, hevc_config: &mut crate::sys::NV_ENC_CONFIG_HEVC) {
        if self.inband_csd_disabled {
            hevc_config.set_disableSPSPPS(1);
        }
        if self.csd_should_repeat {
            hevc_config.set_repeatSPSPPS(1);
        }
    }
}

fn pixel_bit_depth_minus_8(nvenc_format: &crate::sys::NV_ENC_BUFFER_FORMAT) -> u32 {
    // Ignore 10-bit RGB formats:
    //
    // https://github.com/NVIDIA/video-sdk-samples/blob/aa3544dcea2fe63122e4feb83bf805ea40e58dbe/nvEncBroadcastSample/nvEnc/nvCodec/nvEncoder/NvEncoder.cpp#L200
    match nvenc_format {
        crate::sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_YUV420_10BIT
        | crate::sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_YUV444_10BIT => 2,
        _ => 0,
    }
}

fn chroma_format_idc(nvenc_format: &crate::sys::NV_ENC_BUFFER_FORMAT) -> u32 {
    // Contrary to the header that says YUV420 should have chromaFormatIDC = 1, the video SDK
    // sample only changes the chromaFormatIDC for YUV444 and YUV444_10BIT:
    //
    // https://github.com/NVIDIA/video-sdk-samples/blob/aa3544dcea2fe63122e4feb83bf805ea40e58dbe/nvEncBroadcastSample/nvEnc/nvCodec/nvEncoder/NvEncoder.cpp#L189
    //
    // What should be done is to set chromaFormatIDC to 3 for YUV444 and to 1 otherwise (even for
    // non-YUV formats like RGB). Calls to `NvEncGetEncodePresetConfigEx` automatically sets
    // chromaFormatIDC to 1 so *perhaps* zero is not a valid value for chromaFormatIDC.
    match nvenc_format {
        crate::sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_YUV444
        | crate::sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_YUV444_10BIT => 3,
        _ => 1,
    }
}

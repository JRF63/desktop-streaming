use super::raw_encoder_v2::RawEncoder;
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
    pub fn new(
        raw_encoder: &RawEncoder,
        width: u32,
        height: u32,
        display_aspect_ratio: Option<(u32, u32)>,
        refresh_rate_ratio: (u32, u32),
        codec: Codec,
        profile: CodecProfile,
        preset: EncodePreset,
        tuning_info: TuningInfo,
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

        let codec_config = build_encode_config(raw_encoder, codec, profile, preset, tuning_info)?;

        init_params.encodeConfig = Box::into_raw(codec_config);

        Ok(EncodeParams(reconfig_params))
    }

    pub fn initializer(&mut self) -> &mut crate::sys::NV_ENC_INITIALIZE_PARAMS {
        &mut self.0.reInitEncodeParams
    }

    pub fn set_average_bitrate(&mut self, bitrate: u32) {
        let ptr = self.0.reInitEncodeParams.encodeConfig;
        debug_assert!(
            !ptr.is_null(),
            "reInitEncodeParams.encodeConfig should not be null"
        );

        let encoder_config = unsafe { &mut *ptr };
        encoder_config.rcParams.averageBitRate = bitrate
    }
}

fn build_encode_config(
    raw_encoder: &RawEncoder,
    codec: Codec,
    profile: CodecProfile,
    preset: EncodePreset,
    tuning_info: TuningInfo,
) -> Result<Box<crate::sys::NV_ENC_CONFIG>> {
    let encode_guid = codec.into();
    let preset_guid = preset.into();
    let mut preset_config_params = unsafe {
        let mut tmp: MaybeUninit<crate::sys::NV_ENC_PRESET_CONFIG> = MaybeUninit::zeroed();

        let ptr = tmp.as_mut_ptr();

        addr_of_mut!((*ptr).version).write(crate::sys::NV_ENC_PRESET_CONFIG_VER);
        addr_of_mut!((*ptr).presetCfg.version).write(crate::sys::NV_ENC_CONFIG_VER);
        addr_of_mut!((*ptr).presetCfg.profileGUID).write(profile.into());
        raw_encoder.get_encode_preset_config_ex(
            encode_guid,
            preset_guid,
            tuning_info.into(),
            ptr,
        )?;
        tmp.assume_init()
    };

    let codec_config = &mut preset_config_params.presetCfg.encodeCodecConfig;

    match codec {
        Codec::H264 => {
            let h264_config = unsafe { &mut codec_config.h264Config.as_mut() };

            // SPS/PPS would be manually given to the decoder
            h264_config.set_disableSPSPPS(1);

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

            // VPS/SPS/PPS would be manually given to the decoder
            hevc_config.set_disableSPSPPS(1);

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

    Ok(Box::new(preset_config_params.presetCfg))
}

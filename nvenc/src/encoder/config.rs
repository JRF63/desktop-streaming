use super::RawEncoder;
use crate::{Codec, EncoderPreset, Result, TuningInfo};
use std::mem::MaybeUninit;

// TODO: Don't depend on this Windows-specific struct
use windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_DESC;

#[repr(transparent)]
pub(crate) struct EncoderParams(crate::sys::NV_ENC_RECONFIGURE_PARAMS);

impl Drop for EncoderParams {
    fn drop(&mut self) {
        let ptr = self.0.reInitEncodeParams.encodeConfig;
        std::mem::drop(Box::new(ptr));
    }
}

impl EncoderParams {
    pub(crate) fn new(
        raw_encoder: &RawEncoder,
        display_desc: &DXGI_OUTDUPL_DESC,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> Result<Self> {
        let mut reconfig_params: crate::sys::NV_ENC_RECONFIGURE_PARAMS =
            unsafe { MaybeUninit::zeroed().assume_init() };
        reconfig_params.version = crate::sys::NV_ENC_RECONFIGURE_PARAMS_VER;

        let init_params = &mut reconfig_params.reInitEncodeParams;
        init_params.version = crate::sys::NV_ENC_INITIALIZE_PARAMS_VER;
        init_params.encodeGUID = codec.into();
        init_params.presetGUID = preset.into();
        init_params.encodeWidth = display_desc.ModeDesc.Width;
        init_params.encodeHeight = display_desc.ModeDesc.Height;
        let gcd = crate::util::gcd(display_desc.ModeDesc.Width, display_desc.ModeDesc.Height);
        init_params.darWidth = display_desc.ModeDesc.Width / gcd;
        init_params.darHeight = display_desc.ModeDesc.Height / gcd;
        init_params.frameRateNum = display_desc.ModeDesc.RefreshRate.Numerator;
        init_params.frameRateDen = display_desc.ModeDesc.RefreshRate.Denominator;
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

        let codec_config =
            EncoderParams::get_codec_config_for_preset(raw_encoder, codec, preset, tuning_info)?;

        init_params.encodeConfig = Box::into_raw(codec_config);

        Ok(EncoderParams(reconfig_params))
    }

    fn get_codec_config_for_preset(
        raw_encoder: &RawEncoder,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> Result<Box<crate::sys::NV_ENC_CONFIG>> {
        let encode_guid = codec.into();
        let preset_guid = preset.into();
        let mut preset_config_params = unsafe {
            let mut tmp: MaybeUninit<crate::sys::NV_ENC_PRESET_CONFIG> = MaybeUninit::zeroed();
            let mut_ref = &mut *tmp.as_mut_ptr();

            mut_ref.version = crate::sys::NV_ENC_PRESET_CONFIG_VER;
            mut_ref.presetCfg.version = crate::sys::NV_ENC_CONFIG_VER;
            // mut_ref.presetCfg.rcParams.version = crate::sys::NV_ENC_RC_PARAMS_VER;

            raw_encoder.get_encode_preset_config_ex(
                encode_guid,
                preset_guid,
                tuning_info.into(),
                tmp.as_mut_ptr(),
            )?;
            tmp.assume_init()
        };

        let codec_config = &mut preset_config_params.presetCfg;
        match codec {
            Codec::H264 => {
                let h264_config = unsafe { &mut codec_config.encodeCodecConfig.h264Config };

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
                let hevc_config = unsafe { &mut codec_config.encodeCodecConfig.hevcConfig };

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

    pub(crate) fn encode_config(&self) -> &crate::sys::NV_ENC_CONFIG {
        unsafe { &*self.init_params().encodeConfig }
    }

    pub(crate) fn encode_config_mut(&mut self) -> &mut crate::sys::NV_ENC_CONFIG {
        unsafe { &mut *self.init_params_mut().encodeConfig }
    }

    pub(crate) fn init_params(&self) -> &crate::sys::NV_ENC_INITIALIZE_PARAMS {
        &self.0.reInitEncodeParams
    }

    pub(crate) fn init_params_mut(&mut self) -> &mut crate::sys::NV_ENC_INITIALIZE_PARAMS {
        &mut self.0.reInitEncodeParams
    }

    pub(crate) fn reconfig_params(&self) -> &crate::sys::NV_ENC_RECONFIGURE_PARAMS {
        &self.0
    }

    pub(crate) fn reconfig_params_mut(&mut self) -> &mut crate::sys::NV_ENC_RECONFIGURE_PARAMS {
        &mut self.0
    }
}

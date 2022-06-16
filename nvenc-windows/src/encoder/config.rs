use super::RawEncoder;
use crate::{util::IntoNvEncBufferFormat, Codec, EncoderPreset, Result, TuningInfo};
use std::mem::MaybeUninit;

// TODO: Don't depend on this Windows-specific struct
use windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_DESC;

#[repr(transparent)]
pub(crate) struct EncoderParams(nvenc_sys::NV_ENC_RECONFIGURE_PARAMS);

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
        let mut codec_config =
            EncoderParams::get_codec_config_for_preset(raw_encoder, codec, preset, tuning_info)?;

        // https://docs.nvidia.com/video-technologies/video-codec-sdk/nvenc-video-encoder-api-prog-guide/
        // Settings for optimal performance when using `IDXGIOutputDuplication::AcquireNextFrame`
        match codec {
            Codec::H264 => {
                let h264_config = unsafe { &mut codec_config.encodeCodecConfig.h264Config };
                h264_config.set_enableFillerDataInsertion(0);
                h264_config.set_outputBufferingPeriodSEI(0);
                h264_config.set_outputPictureTimingSEI(0);
                h264_config.set_outputAUD(0);
                h264_config.set_outputFramePackingSEI(0);
                h264_config.set_outputRecoveryPointSEI(0);
                h264_config.set_enableScalabilityInfoSEI(0);
                h264_config.set_disableSVCPrefixNalu(1);
                // SPS/PPS would be manually given to the decoder
                h264_config.set_disableSPSPPS(1);
            }
            Codec::Hevc => {
                let hevc_config = unsafe { &mut codec_config.encodeCodecConfig.hevcConfig };
                hevc_config.set_enableFillerDataInsertion(0);
                hevc_config.set_outputBufferingPeriodSEI(0);
                hevc_config.set_outputPictureTimingSEI(0);
                hevc_config.set_outputAUD(0);
                hevc_config.set_enableAlphaLayerEncoding(0);
                // VPS/SPS/PPS would be manually given to the decoder
                hevc_config.set_disableSPSPPS(1);
            }
        }

        let mut init_params: nvenc_sys::NV_ENC_INITIALIZE_PARAMS =
            unsafe { MaybeUninit::zeroed().assume_init() };
        init_params.version = nvenc_sys::NV_ENC_INITIALIZE_PARAMS_VER;
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
        init_params.encodeConfig = Box::into_raw(codec_config);
        init_params.tuningInfo = tuning_info.into();
        init_params.bufferFormat = display_desc.ModeDesc.Format.into_nvenc_buffer_format();

        // Settings for optimal performance same as above
        #[cfg(windows)]
        {
            init_params.enableEncodeAsync = 1;
        }
        init_params.set_enableOutputInVidmem(0);

        let mut tmp: nvenc_sys::NV_ENC_RECONFIGURE_PARAMS =
            unsafe { MaybeUninit::zeroed().assume_init() };
        tmp.version = nvenc_sys::NV_ENC_RECONFIGURE_PARAMS_VER;
        tmp.reInitEncodeParams = init_params;

        Ok(EncoderParams(tmp))
    }

    fn get_codec_config_for_preset(
        raw_encoder: &RawEncoder,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> Result<Box<nvenc_sys::NV_ENC_CONFIG>> {
        let encode_guid = codec.into();
        let preset_guid = preset.into();
        let preset_config_params = unsafe {
            let mut tmp: MaybeUninit<nvenc_sys::NV_ENC_PRESET_CONFIG> = MaybeUninit::zeroed();
            let mut_ref = &mut *tmp.as_mut_ptr();
            mut_ref.version = nvenc_sys::NV_ENC_PRESET_CONFIG_VER;
            mut_ref.presetCfg.version = nvenc_sys::NV_ENC_CONFIG_VER;

            raw_encoder.get_encode_preset_config_ex(
                encode_guid,
                preset_guid,
                tuning_info.into(),
                tmp.as_mut_ptr(),
            )?;
            tmp.assume_init()
        };

        Ok(Box::new(preset_config_params.presetCfg))
    }

    // pub(crate) fn encode_config(&self) -> &nvenc_sys::NV_ENC_CONFIG {
    //     unsafe { &*self.init_params().encodeConfig }
    // }

    pub(crate) fn encode_config_mut(&mut self) -> &mut nvenc_sys::NV_ENC_CONFIG {
        unsafe { &mut *self.init_params_mut().encodeConfig }
    }

    pub(crate) fn init_params(&self) -> &nvenc_sys::NV_ENC_INITIALIZE_PARAMS {
        &self.0.reInitEncodeParams
    }

    pub(crate) fn init_params_mut(&mut self) -> &mut nvenc_sys::NV_ENC_INITIALIZE_PARAMS {
        &mut self.0.reInitEncodeParams
    }

    pub(crate) fn reconfig_params_mut(&mut self) -> &mut nvenc_sys::NV_ENC_RECONFIGURE_PARAMS {
        &mut self.0
    }
}

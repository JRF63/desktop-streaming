mod init;
mod input;
mod output;
mod queries;

use self::init::*;
use crate::{error::NvEncError, nvenc_function, Codec, EncoderPreset, TuningInfo};
use input::{EncoderInput, EncoderInputReturn};
use output::EncoderOutput;
use std::{cell::UnsafeCell, mem::MaybeUninit, os::raw::c_void, ptr::NonNull, sync::Arc};
use windows::Win32::Graphics::{
    Direct3D11::{ID3D11Device, ID3D11Texture2D},
    Dxgi::DXGI_OUTDUPL_DESC,
};

pub type Result<T> = std::result::Result<T, NvEncError>;

pub(crate) struct EncoderBuffers {
    registered_resource: NonNull<c_void>,
    input_ptr: UnsafeCell<nvenc_sys::NV_ENC_INPUT_PTR>,
    output_ptr: NonNull<c_void>,
    event_obj: EventObject,
}

unsafe impl Sync for EncoderBuffers {}

impl EncoderBuffers {
    pub(crate) fn cleanup(
        &mut self,
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
    ) {
        // TODO: Prob should log the errors instead of ignoring them.
        unsafe {
            (functions.nvEncUnmapInputResource.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                *self.input_ptr.get(),
            );
            (functions.nvEncUnregisterResource.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.registered_resource.as_ptr(),
            );
            (functions.nvEncDestroyBitstreamBuffer.unwrap_unchecked())(
                raw_encoder.as_ptr(),
                self.output_ptr.as_ptr(),
            );
            let _ignore = unregister_async_event(functions, raw_encoder, &self.event_obj);
        }
    }
}

// TODO: Pull out the function list into a global struct?
pub(crate) struct NvidiaEncoder {
    raw_encoder: NonNull<c_void>,
    functions: nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    input_textures: ID3D11Texture2D,
    buffers: Vec<EncoderBuffers>,
    #[allow(dead_code)]
    library: Library,
}

impl Drop for NvidiaEncoder {
    fn drop(&mut self) {
        for buffer in &mut self.buffers {
            buffer.cleanup(&self.functions, self.raw_encoder);
        }
        unsafe {
            (self.functions.nvEncDestroyEncoder.unwrap())(self.raw_encoder.as_ptr());
        }
    }
}

// TODO: `Sync` and `Send` are technically wrong
unsafe impl Sync for NvidiaEncoder {}
unsafe impl Send for NvidiaEncoder {}

impl NvidiaEncoder {
    pub(crate) fn new(
        device: ID3D11Device,
        display_desc: &DXGI_OUTDUPL_DESC,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
        buf_size: usize,
    ) -> anyhow::Result<(Self, EncoderParams)> {
        let library = Library::load("nvEncodeAPI64.dll")?;
        if !is_version_supported(&library)? {
            return Err(anyhow::anyhow!(
                "NVENC version is not supported by the installed driver"
            ));
        }
        let functions = get_function_list(&library)?;
        let raw_encoder = open_encode_session(&functions, device.clone())?;

        let mut encoder_params = EncoderParams::new(
            &functions,
            raw_encoder,
            display_desc,
            codec,
            preset,
            tuning_info,
        )?;
        unsafe {
            nvenc_function!(
                functions.nvEncInitializeEncoder,
                raw_encoder.as_ptr(),
                &mut encoder_params.0.reInitEncodeParams
            );
        }

        let input_textures = create_texture_buffer(&device, display_desc, buf_size)?;

        let mut buffers = Vec::with_capacity(buf_size);

        // Using a closure for graceful cleanup
        let mut inner = || -> anyhow::Result<()> {
            for i in 0..buf_size {
                let registered_resource =
                    register_resource(&functions, raw_encoder, input_textures.clone(), i as u32)?;
                let output_ptr = create_output_buffers(&functions, raw_encoder)?;
                let event_obj = EventObject::new()?;
                register_async_event(&functions, raw_encoder, &event_obj);

                buffers.push(EncoderBuffers {
                    registered_resource,
                    input_ptr: UnsafeCell::new(std::ptr::null_mut()),
                    output_ptr,
                    event_obj,
                });
            }
            Ok(())
        };

        if let Err(e) = inner() {
            for mut buffer in buffers {
                buffer.cleanup(&functions, raw_encoder);
            }
            return Err(e);
        }

        Ok((
            NvidiaEncoder {
                raw_encoder,
                functions,
                input_textures,
                buffers,
                library,
            },
            encoder_params,
        ))
    }
}

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
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
        display_desc: &DXGI_OUTDUPL_DESC,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> Result<Self> {
        let mut encoder_config =
            EncoderParams::create_config(&functions, raw_encoder, codec, preset, tuning_info)?;

        // https://docs.nvidia.com/video-technologies/video-codec-sdk/nvenc-video-encoder-api-prog-guide/
        // Settings for optimal performance when using `IDXGIOutputDuplication::AcquireNextFrame`
        match codec {
            Codec::H264 => {
                let h264_config = unsafe { &mut encoder_config.encodeCodecConfig.h264Config };
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
                let hevc_config = unsafe { &mut encoder_config.encodeCodecConfig.hevcConfig };
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
        init_params.darWidth = display_desc.ModeDesc.Width;
        init_params.darHeight = display_desc.ModeDesc.Height;
        init_params.frameRateNum = display_desc.ModeDesc.RefreshRate.Numerator;
        init_params.frameRateDen = display_desc.ModeDesc.RefreshRate.Denominator;
        init_params.enablePTD = 1; // TODO: Currently enabling picture type detection for convenience
        init_params.encodeConfig = Box::into_raw(encoder_config);
        init_params.tuningInfo = tuning_info.into();
        init_params.bufferFormat = crate::util::dxgi_to_nv_format(display_desc.ModeDesc.Format);

        // Settings for optimal performance same as above
        init_params.enableEncodeAsync = 1;
        init_params.set_enableOutputInVidmem(0);

        let mut tmp: nvenc_sys::NV_ENC_RECONFIGURE_PARAMS =
            unsafe { MaybeUninit::zeroed().assume_init() };
        tmp.version = nvenc_sys::NV_ENC_RECONFIGURE_PARAMS_VER;
        tmp.reInitEncodeParams = init_params;

        Ok(EncoderParams(tmp))
    }

    fn create_config(
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> Result<Box<nvenc_sys::NV_ENC_CONFIG>> {
        let encode_guid = codec.into();
        let preset_guid = preset.into();
        let preset_config_params = {
            unsafe {
                let mut tmp: MaybeUninit<nvenc_sys::NV_ENC_PRESET_CONFIG> = MaybeUninit::zeroed();
                let mut_ref = &mut *tmp.as_mut_ptr();
                mut_ref.version = nvenc_sys::NV_ENC_PRESET_CONFIG_VER;
                mut_ref.presetCfg.version = nvenc_sys::NV_ENC_CONFIG_VER;
                nvenc_function!(
                    functions.nvEncGetEncodePresetConfigEx,
                    raw_encoder.as_ptr(),
                    encode_guid,
                    preset_guid,
                    tuning_info.into(),
                    tmp.as_mut_ptr()
                );
                tmp.assume_init()
            }
        };
        Ok(Box::new(preset_config_params.presetCfg))
    }

    pub(crate) fn encode_config(&self) -> &nvenc_sys::NV_ENC_CONFIG {
        unsafe { &*self.init_params().encodeConfig }
    }

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

pub fn create_encoder(
    device: ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
    codec: Codec,
    preset: EncoderPreset,
    tuning_info: TuningInfo,
    buf_size: usize,
) -> (EncoderInput, EncoderOutput) {
    let mut device_context = None;
    unsafe {
        device.GetImmediateContext(&mut device_context);
    }

    let (encoder, encoder_params) =
        NvidiaEncoder::new(device, display_desc, codec, preset, tuning_info, buf_size).unwrap();
    let encoder = Arc::new(encoder);

    let EncoderInputReturn {
        encoder_input,
        avail_indices_sender,
        occupied_indices_receiver,
    } = EncoderInput::new(
        encoder.clone(),
        device_context.unwrap(),
        encoder_params,
        buf_size,
    );

    let encoder_output =
        EncoderOutput::new(encoder, occupied_indices_receiver, avail_indices_sender);

    (encoder_input, encoder_output)
}

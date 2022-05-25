mod init;
mod input;
mod output;
mod queries;

use self::init::{
    create_output_buffers, create_texture_buffer,
    get_function_list, is_version_supported, open_encode_session, register_resource, Library, EventObject
};
use crate::{error::NvEncError, nvenc_function, Codec, EncoderPreset, TuningInfo};
use crossbeam_channel::{Receiver, Sender};
use input::{EncoderInput, EncoderInputReturn};
use output::EncoderOutput;
use std::{cell::UnsafeCell, mem::MaybeUninit, os::raw::c_void, ptr::NonNull, sync::Arc};
use windows::Win32::{
    Graphics::{
        Direct3D11::{ID3D11Device, ID3D11Texture2D},
        Dxgi::{IDXGIResource, DXGI_OUTDUPL_DESC},
    },
};

pub type Result<T> = std::result::Result<T, NvEncError>;

pub(crate) struct EncoderBuffers {
    registered_resource: NonNull<c_void>,
    input_ptr: UnsafeCell<nvenc_sys::NV_ENC_INPUT_PTR>,
    output_ptr: NonNull<c_void>,
    event_obj: EventObject,
}

unsafe impl Sync for EncoderBuffers {}

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
        // TODO: Prob should log the errors instead of ignoring them.
        for io in &self.buffers {
            unsafe {
                (self.functions.nvEncUnmapInputResource.unwrap())(
                    self.raw_encoder.as_ptr(),
                    *io.input_ptr.get(),
                );
                (self.functions.nvEncUnregisterResource.unwrap())(
                    self.raw_encoder.as_ptr(),
                    io.registered_resource.as_ptr(),
                );
                (self.functions.nvEncDestroyBitstreamBuffer.unwrap())(
                    self.raw_encoder.as_ptr(),
                    io.output_ptr.as_ptr(),
                );
            }
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
    ) -> Option<(Self, nvenc_sys::NV_ENC_INITIALIZE_PARAMS)> {
        // TODO: Log errors or bubble them up.
        let library = Library::load("nvEncodeAPI64.dll").ok()?;
        if !is_version_supported(&library)? {
            eprintln!("Version not supported.");
            return None;
        }
        let functions = get_function_list(&library)?;
        let raw_encoder = open_encode_session(&functions, device.clone())?;

        let (init_params, encoder_config) = NvidiaEncoder::init_encoder(
            &functions,
            raw_encoder,
            display_desc,
            codec,
            preset,
            tuning_info,
        )
        .ok()?;
        // TODO: `encoder_config`

        let input_textures = create_texture_buffer(&device, display_desc, buf_size).ok()?;

        let mut buffers = Vec::with_capacity(buf_size);

        // TODO: Error on one would fail to free/release the preceding items
        for i in 0..buf_size {
            let registered_resource =
                register_resource(&functions, raw_encoder, input_textures.clone(), i as u32)
                    .ok()?;
            let output_ptr = create_output_buffers(&functions, raw_encoder.clone()).ok()?;
            let event_obj = EventObject::new().ok()?;

            buffers.push(EncoderBuffers {
                registered_resource,
                input_ptr: UnsafeCell::new(std::ptr::null_mut()),
                output_ptr,
                event_obj,
            });
        }

        Some((
            NvidiaEncoder {
                raw_encoder,
                functions,
                input_textures,
                buffers,
                library,
            },
            init_params,
        ))
    }

    fn create_encoder_config(
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> Result<nvenc_sys::NV_ENC_CONFIG> {
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
        Ok(preset_config_params.presetCfg)
    }

    fn init_encoder(
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
        display_desc: &DXGI_OUTDUPL_DESC,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> Result<(
        nvenc_sys::NV_ENC_INITIALIZE_PARAMS,
        nvenc_sys::NV_ENC_CONFIG,
    )> {
        let mut encoder_config = NvidiaEncoder::create_encoder_config(
            &functions,
            raw_encoder,
            codec,
            preset,
            tuning_info,
        )?;

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
        init_params.encodeConfig = &mut encoder_config; // BUG
        init_params.tuningInfo = tuning_info.into();
        init_params.bufferFormat = crate::util::dxgi_to_nv_format(display_desc.ModeDesc.Format);

        // https://docs.nvidia.com/video-technologies/video-codec-sdk/nvenc-video-encoder-api-prog-guide/
        // Settings for optimal performance when using `IDXGIOutputDuplication::AcquireNextFrame`
        init_params.enableEncodeAsync = 1;
        init_params.set_enableOutputInVidmem(0);

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
            }
            Codec::Hevc => {
                let hevc_config = unsafe { &mut encoder_config.encodeCodecConfig.hevcConfig };
                hevc_config.set_enableFillerDataInsertion(0);
                hevc_config.set_outputBufferingPeriodSEI(0);
                hevc_config.set_outputPictureTimingSEI(0);
                hevc_config.set_outputAUD(0);
                hevc_config.set_enableAlphaLayerEncoding(0);
            }
        }

        unsafe {
            nvenc_function!(
                functions.nvEncInitializeEncoder,
                raw_encoder.as_ptr(),
                &mut init_params
            );
        }

        Ok((init_params, encoder_config))
    }
}

pub fn create_encoder(
    device: ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
    codec: Codec,
    preset: EncoderPreset,
    tuning_info: TuningInfo,
    buf_size: usize,
) -> (
    EncoderInput,
    EncoderOutput,
    Sender<IDXGIResource>,
    Receiver<()>,
) {
    let mut device_context = None;
    unsafe {
        device.GetImmediateContext(&mut device_context);
    }

    let (encoder, init_params) =
        NvidiaEncoder::new(device, display_desc, codec, preset, tuning_info, buf_size).unwrap();
    let encoder = Arc::new(encoder);

    let EncoderInputReturn {
        encoder_input,
        frame_sender,
        copy_complete_receiver,
        avail_indices_sender,
        occupied_indices_receiver,
    } = EncoderInput::new(
        encoder.clone(),
        device_context.unwrap(),
        init_params,
        buf_size,
    );

    let encoder_output =
        EncoderOutput::new(encoder, occupied_indices_receiver, avail_indices_sender);

    (
        encoder_input,
        encoder_output,
        frame_sender,
        copy_complete_receiver,
    )
}

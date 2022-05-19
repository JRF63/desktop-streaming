mod init;
mod input;
mod output;
mod queries;

use self::init::{
    create_event_object, create_output_buffers, create_texture_buffer, destroy_event_object,
    free_library, get_function_list, is_version_supported, load_library, open_encode_session,
    register_resource,
};
use crate::{
    error::NvEncError,
    guids::{Codec, EncoderPreset},
    nvenc_function,
};
use crossbeam_channel::{Receiver, Sender};
use input::{EncoderInput, EncoderInputReturn};
use output::EncoderOutput;
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull, sync::Arc, cell::UnsafeCell};
use windows::Win32::{
    Foundation::{HANDLE, HINSTANCE},
    Graphics::{
        Direct3D11::{ID3D11Device, ID3D11Texture2D},
        Dxgi::{IDXGIResource, DXGI_OUTDUPL_DESC},
    },
};

pub type Result<T> = std::result::Result<T, NvEncError>;

pub(crate) struct EncoderIO {
    texture: ID3D11Texture2D,
    registered_resource: NonNull<c_void>,
    input_ptr: UnsafeCell<nvenc_sys::NV_ENC_INPUT_PTR>,
    output_ptr: NonNull<c_void>,
    event_obj: HANDLE,
}

unsafe impl Sync for EncoderIO {}

// TODO: Pull out the function list into a global struct?
pub(crate) struct NvidiaEncoder<const BUF_SIZE: usize> {
    raw_encoder: NonNull<c_void>,
    functions: nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    io: [EncoderIO; BUF_SIZE],
    library: HINSTANCE,
}

impl<const BUF_SIZE: usize> Drop for NvidiaEncoder<BUF_SIZE> {
    fn drop(&mut self) {
        // TODO: Prob should log the errors instead of ignoring them.
        for io in &self.io {
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
                destroy_event_object(io.event_obj);
            }
        }
        unsafe {
            (self.functions.nvEncDestroyEncoder.unwrap())(self.raw_encoder.as_ptr());
        }
        free_library(self.library);
    }
}

// TODO: `Sync` and `Send` are technically wrong
unsafe impl<const BUF_SIZE: usize> Sync for NvidiaEncoder<BUF_SIZE> {}
unsafe impl<const BUF_SIZE: usize> Send for NvidiaEncoder<BUF_SIZE> {}

impl<const BUF_SIZE: usize> NvidiaEncoder<BUF_SIZE> {
    pub(crate) fn new(
        device: ID3D11Device,
        display_desc: &DXGI_OUTDUPL_DESC,
    ) -> Option<(Self, nvenc_sys::NV_ENC_INITIALIZE_PARAMS)> {
        // TODO: Log errors or bubble them up.
        let library = load_library("nvEncodeAPI64.dll")?;
        if !is_version_supported(library)? {
            eprintln!("Version not supported.");
            return None;
        }
        let functions = get_function_list(library)?;
        let raw_encoder = open_encode_session(&functions, device.clone())?;

        // TODO: dummy
        let init_params = NvidiaEncoder::<BUF_SIZE>::dummy_init_encoder(
            &functions,
            raw_encoder.clone(),
            display_desc,
        )
        .ok()?;

        let mut io: MaybeUninit<[EncoderIO; BUF_SIZE]> = MaybeUninit::uninit();

        // TODO: Error on one would fail to free/release the preceding items
        unsafe {
            let ptr = (&mut *io.as_mut_ptr()).as_mut_ptr();
            for i in 0..BUF_SIZE {
                let texture = create_texture_buffer(&device, display_desc).ok()?;
                let registered_resource =
                    register_resource(&functions, raw_encoder, texture.clone()).ok()?;
                let output_ptr = create_output_buffers(&functions, raw_encoder.clone()).ok()?;
                let event_obj = create_event_object().ok()?;

                ptr.offset(i as isize).write(EncoderIO {
                    texture,
                    registered_resource,
                    input_ptr: UnsafeCell::new(std::ptr::null_mut()),
                    output_ptr,
                    event_obj,
                });
            }
        }

        Some((
            NvidiaEncoder {
                raw_encoder,
                functions,
                io: unsafe { io.assume_init() },
                library,
            },
            init_params,
        ))
    }

    fn dummy_init_encoder(
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
        display_desc: &DXGI_OUTDUPL_DESC,
    ) -> Result<nvenc_sys::NV_ENC_INITIALIZE_PARAMS> {
        let encode_guid = Codec::H264.into();
        let preset_guid = EncoderPreset::P2.into();
        let tuning_info = nvenc_sys::NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY;
        let mut preset_config_params = {
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
                    tuning_info,
                    tmp.as_mut_ptr()
                );
                tmp.assume_init()
            }
        };

        // TODO: Modify `preset_config_params.presetCfg`

        let mut init_params: nvenc_sys::NV_ENC_INITIALIZE_PARAMS =
            unsafe { MaybeUninit::zeroed().assume_init() };
        init_params.version = nvenc_sys::NV_ENC_INITIALIZE_PARAMS_VER;
        init_params.encodeGUID = encode_guid;
        init_params.presetGUID = preset_guid;
        init_params.encodeWidth = display_desc.ModeDesc.Width;
        init_params.encodeHeight = display_desc.ModeDesc.Height;
        init_params.darWidth = display_desc.ModeDesc.Width;
        init_params.darHeight = display_desc.ModeDesc.Height;
        init_params.frameRateNum = display_desc.ModeDesc.RefreshRate.Numerator;
        init_params.frameRateDen = display_desc.ModeDesc.RefreshRate.Denominator;
        init_params.enableEncodeAsync = 1;
        init_params.enablePTD = 1; // TODO: Currently enabling picture type detection for convenience
        init_params.encodeConfig = &mut preset_config_params.presetCfg;
        init_params.tuningInfo = tuning_info;
        init_params.bufferFormat = crate::util::dxgi_to_nv_format(display_desc.ModeDesc.Format);

        unsafe {
            nvenc_function!(
                functions.nvEncInitializeEncoder,
                raw_encoder.as_ptr(),
                &mut init_params
            );
        }

        Ok(init_params)
    }
}

pub fn create_encoder<const BUF_SIZE: usize>(
    device: ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
) -> (
    EncoderInput<BUF_SIZE>,
    EncoderOutput<BUF_SIZE>,
    Sender<IDXGIResource>,
    Receiver<()>,
) {
    let mut device_context = None;
    unsafe {
        device.GetImmediateContext(&mut device_context);
    }

    let (encoder, init_params) = NvidiaEncoder::<BUF_SIZE>::new(device, display_desc).unwrap();
    let encoder = Arc::new(encoder);

    let EncoderInputReturn {
        encoder_input,
        frame_sender,
        copy_complete_receiver,
        avail_indices_sender,
        occupied_indices_receiver,
    } = EncoderInput::new(encoder.clone(), device_context.unwrap(), init_params);

    let encoder_output =
        EncoderOutput::new(encoder, occupied_indices_receiver, avail_indices_sender);

    (
        encoder_input,
        encoder_output,
        frame_sender,
        copy_complete_receiver,
    )
}

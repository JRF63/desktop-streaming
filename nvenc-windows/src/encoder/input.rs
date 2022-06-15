use super::{config::EncoderParams, NvidiaEncoder, Result};
use crate::nvenc_function;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull, sync::Arc};
use windows::{
    core::Interface,
    Win32::Graphics::{
        Direct3D11::{ID3D11DeviceContext, ID3D11Texture2D},
        Dxgi::IDXGIResource,
    },
};

pub struct EncoderInput {
    encoder: Arc<NvidiaEncoder>,
    device_context: ID3D11DeviceContext,
    pic_params: nvenc_sys::NV_ENC_PIC_PARAMS,
    encoder_params: EncoderParams,
}

unsafe impl Send for EncoderInput {}

impl EncoderInput {
    pub(crate) fn new(
        encoder: Arc<NvidiaEncoder>,
        device_context: ID3D11DeviceContext,
        encoder_params: EncoderParams,
    ) -> Self {
        let pic_params = {
            let mut tmp: nvenc_sys::NV_ENC_PIC_PARAMS =
                unsafe { MaybeUninit::zeroed().assume_init() };
            tmp.version = nvenc_sys::NV_ENC_PIC_PARAMS_VER;
            tmp.inputWidth = encoder_params.init_params().encodeWidth;
            tmp.inputHeight = encoder_params.init_params().encodeHeight;
            tmp.inputPitch = tmp.inputWidth;
            tmp.bufferFmt = encoder_params.init_params().bufferFormat;
            tmp.pictureStruct = nvenc_sys::NV_ENC_PIC_STRUCT::NV_ENC_PIC_STRUCT_FRAME;
            tmp
        };

        EncoderInput {
            encoder,
            device_context,
            pic_params,
            encoder_params,
        }
    }

    pub fn end_encode(&mut self) {
        todo!()
        // self.pic_params.encodePicFlags = nvenc_sys::NV_ENC_PIC_FLAG_EOS;
    }

    pub(crate) fn reconfigure_params(&mut self) -> Result<()> {
        unsafe {
            nvenc_function!(
                self.encoder.functions.nvEncReconfigureEncoder,
                self.encoder.raw_encoder.as_ptr(),
                self.encoder_params.reconfig_params_mut()
            );
        }

        Ok(())
    }

    pub fn update_average_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.encoder_params
            .encode_config_mut()
            .rcParams
            .averageBitRate = bitrate;

        self.reconfigure_params()
    }

    pub fn get_codec_specific_data(&self) -> Result<Vec<u8>> {
        unsafe {
            let mut buffer = vec![0; 1024];
            let mut bytes_written = 0;
            let mut params: nvenc_sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD =
                MaybeUninit::zeroed().assume_init();
            params.version = nvenc_sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD_VER;
            params.inBufferSize = buffer.len() as u32;
            params.spsppsBuffer = buffer.as_mut_ptr().cast();
            params.outSPSPPSPayloadSize = &mut bytes_written;

            nvenc_function!(
                self.encoder.functions.nvEncGetSequenceParams,
                self.encoder.raw_encoder.as_ptr(),
                &mut params
            );

            buffer.truncate(bytes_written as usize);
            Ok(buffer)
        }
    }

    pub fn encode_frame(&mut self, frame: IDXGIResource, timestamp: u32) -> Result<()> {
        let pic_params = &mut self.pic_params;
        let device_context = &self.device_context;
        let input_textures = &self.encoder.input_textures;
        let functions = &self.encoder.functions;
        let raw_encoder = self.encoder.raw_encoder;
        
        self.encoder.buffer.writer_access(|index, buffer| {
            EncoderInput::copy_input_frame(device_context, input_textures, &frame, index);

            buffer.input_ptr = EncoderInput::map_input(
                functions,
                raw_encoder,
                buffer.registered_resource.as_ptr(),
            )
            .unwrap();
            pic_params.inputBuffer = buffer.input_ptr;
            pic_params.outputBitstream = buffer.output_ptr.as_ptr();
            pic_params.completionEvent = buffer.event_obj.as_ptr();
        });
        // Already copied
        std::mem::drop(frame);

        // Used for invalidation of frames
        self.pic_params.inputTimeStamp = timestamp as u64;

        unsafe {
            nvenc_function!(
                self.encoder.functions.nvEncEncodePicture,
                self.encoder.raw_encoder.as_ptr(),
                &mut self.pic_params
            );
        }

        Ok(())
    }

    /// Copies the passed resource to the internal texture buffer.
    fn copy_input_frame(
        device_context: &ID3D11DeviceContext,
        input_textures: &ID3D11Texture2D,
        frame: &IDXGIResource,
        subresource_index: usize,
    ) {
        unsafe {
            // `IDXGIResource` to `ID3D11Texture2D` should never fail
            let acquired_image: ID3D11Texture2D = frame.cast().unwrap_unchecked();
            device_context.CopySubresourceRegion(
                input_textures,
                subresource_index as u32,
                0,
                0,
                0,
                &acquired_image,
                0,
                std::ptr::null(),
            );
        }
    }

    /// Does not seem to function as a sync barrier. Texture copy only syncs on call to
    /// `nvEncEncodePicture` if async encode is enabled.
    #[inline]
    fn map_input(
        functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        raw_encoder: NonNull<c_void>,
        registered_resource: nvenc_sys::NV_ENC_REGISTERED_PTR,
    ) -> Result<nvenc_sys::NV_ENC_INPUT_PTR> {
        let mut map_input_resource_params: nvenc_sys::NV_ENC_MAP_INPUT_RESOURCE =
            unsafe { MaybeUninit::zeroed().assume_init() };
        map_input_resource_params.version = nvenc_sys::NV_ENC_MAP_INPUT_RESOURCE_VER;
        map_input_resource_params.registeredResource = registered_resource;

        unsafe {
            nvenc_function!(
                functions.nvEncMapInputResource,
                raw_encoder.as_ptr(),
                &mut map_input_resource_params
            );
        }
        Ok(map_input_resource_params.mappedResource)
    }
}

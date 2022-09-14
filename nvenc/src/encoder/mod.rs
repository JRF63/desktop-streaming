mod buffer;
mod config;
mod event;
mod library;
mod output;
mod queries;
mod raw;
mod shared;

use self::{
    config::EncoderParams,
    output::EncoderOutput,
    raw::RawEncoder,
    shared::{encoder_channel, NvidiaEncoderReader, NvidiaEncoderWriter},
    event::{EventObject, EventObjectTrait}
};
use crate::{Codec, EncoderPreset, Result, TuningInfo};
use std::{mem::MaybeUninit, ops::Deref};

use windows::{
    core::Interface,
    Win32::Graphics::{
        Direct3D11::{ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D},
        Dxgi::{IDXGIResource, DXGI_OUTDUPL_DESC},
    },
};

use crate::os::windows::create_texture_buffer;

/// Size of the ring buffer that is shared between the input and output
const ENCODER_BUFFER_SIZE: usize = 8;

pub struct NvidiaEncoder {
    writer: NvidiaEncoderWriter<ENCODER_BUFFER_SIZE>,
    device_context: ID3D11DeviceContext,
    buffer_texture: ID3D11Texture2D,
    encode_pic_params: crate::sys::NV_ENC_PIC_PARAMS,
    encoder_params: EncoderParams,
}

impl Drop for NvidiaEncoder {
    fn drop(&mut self) {
        let _ = self.end_encode();
    }
}

impl NvidiaEncoder {
    pub fn new(
        writer: NvidiaEncoderWriter<ENCODER_BUFFER_SIZE>,
        device_context: ID3D11DeviceContext,
        buffer_texture: ID3D11Texture2D,
        encoder_params: EncoderParams,
    ) -> Self {
        let pic_params = {
            let mut tmp: crate::sys::NV_ENC_PIC_PARAMS =
                unsafe { MaybeUninit::zeroed().assume_init() };
            tmp.version = crate::sys::NV_ENC_PIC_PARAMS_VER;
            tmp.inputWidth = encoder_params.init_params().encodeWidth;
            tmp.inputHeight = encoder_params.init_params().encodeHeight;
            tmp.inputPitch = tmp.inputWidth;
            tmp.bufferFmt = encoder_params.init_params().bufferFormat;
            tmp.pictureStruct = crate::sys::NV_ENC_PIC_STRUCT::NV_ENC_PIC_STRUCT_FRAME;
            tmp
        };

        println!("NV_ENC_INITIALIZE_PARAMS ----");
        println!("{:?}", &encoder_params.reconfig_params());
        println!("\nNV_ENC_CONFIG ----");
        let c = encoder_params.encode_config();
        println!("version: {:?}", &c.version);
        println!("profileGUID: {:?}", &c.profileGUID);
        println!("gopLength: {:?}", &c.gopLength);
        println!("frameIntervalP: {:?}", &c.frameIntervalP);
        println!("monoChromeEncoding: {:?}", &c.monoChromeEncoding);
        println!("frameFieldMode: {:?}", &c.frameFieldMode);
        println!("mvPrecision: {:?}", &c.mvPrecision);
        println!("\nNV_ENC_RC_PARAMS ----");
        println!("{:?}", &c.rcParams);
        println!("\nNV_ENC_CONFIG_H264 ----");
        println!("{:?}", unsafe { &c.encodeCodecConfig.h264Config });

        NvidiaEncoder {
            writer,
            device_context,
            buffer_texture,
            encode_pic_params: pic_params,
            encoder_params,
        }
    }

    pub fn reconfigure_params(&mut self) -> Result<()> {
        unsafe {
            self.writer
                .reconfigure_encoder(self.encoder_params.reconfig_params_mut())?;
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
        let mut buffer = vec![0; 1024];
        let mut bytes_written = 0;
        unsafe {
            let mut sequence_param_payload: crate::sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD =
                MaybeUninit::zeroed().assume_init();
            sequence_param_payload.version = crate::sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD_VER;
            sequence_param_payload.inBufferSize = buffer.len() as u32;
            sequence_param_payload.spsppsBuffer = buffer.as_mut_ptr().cast();
            sequence_param_payload.outSPSPPSPayloadSize = &mut bytes_written;

            self.writer
                .get_sequence_params(&mut sequence_param_payload)?;
        }
        buffer.truncate(bytes_written as usize);
        Ok(buffer)
    }

    pub fn encode_frame<F>(
        &mut self,
        frame: IDXGIResource,
        timestamp: u64,
        mut post_copy_op: F,
    ) -> Result<()>
    where
        F: FnMut(),
    {
        let pic_params = &mut self.encode_pic_params;
        let device_context = &self.device_context;
        let input_textures = &self.buffer_texture;
        let raw_encoder: &RawEncoder = self.writer.deref();

        self.writer.write(frame, |index, buffer, frame| {
            NvidiaEncoder::copy_input_frame(device_context, input_textures, frame, index);
            post_copy_op();

            buffer.mapped_input =
                NvidiaEncoder::map_input(raw_encoder, buffer.registered_resource.as_ptr())?;
            pic_params.inputBuffer = buffer.mapped_input;
            pic_params.outputBitstream = buffer.output_buffer.as_ptr();
            pic_params.completionEvent = buffer.event_obj.as_ptr();
            Ok(())
        })?;

        // Used for invalidation of frames
        self.encode_pic_params.inputTimeStamp = timestamp;

        unsafe {
            self.writer.encode_picture(&mut self.encode_pic_params)?;
        }

        Ok(())
    }

    fn end_encode(&mut self) -> Result<()> {
        // TODO: Signal EOS to the output via an AtomicBool or something
        let pic_params = &mut self.encode_pic_params;

        self.writer.write((), |_, buffer, ()| {
            pic_params.inputBuffer = std::ptr::null_mut();
            pic_params.outputBitstream = std::ptr::null_mut();
            pic_params.completionEvent = buffer.event_obj.as_ptr();
            pic_params.encodePicFlags = crate::sys::NV_ENC_PIC_FLAGS::NV_ENC_PIC_FLAG_EOS as u32;
            Ok(())
        })?;

        unsafe {
            self.writer.encode_picture(&mut self.encode_pic_params)?;
        }

        Ok(())
    }

    /// Copies the passed resource to the internal texture buffer.
    fn copy_input_frame(
        device_context: &ID3D11DeviceContext,
        input_textures: &ID3D11Texture2D,
        frame: IDXGIResource,
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
    fn map_input(
        raw_encoder: &RawEncoder,
        registered_resource: crate::sys::NV_ENC_REGISTERED_PTR,
    ) -> Result<crate::sys::NV_ENC_INPUT_PTR> {
        let mut map_input_resource_params: crate::sys::NV_ENC_MAP_INPUT_RESOURCE =
            unsafe { MaybeUninit::zeroed().assume_init() };
        map_input_resource_params.version = crate::sys::NV_ENC_MAP_INPUT_RESOURCE_VER;
        map_input_resource_params.registeredResource = registered_resource;

        unsafe {
            raw_encoder.map_input_resource(&mut map_input_resource_params)?;
        }
        Ok(map_input_resource_params.mappedResource)
    }
}

pub fn create_encoder(
    device: ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
    codec: Codec,
    preset: EncoderPreset,
    tuning_info: TuningInfo,
) -> (NvidiaEncoder, EncoderOutput) {
    let mut device_context = None;
    unsafe {
        device.GetImmediateContext(&mut device_context);
    }

    let buffer_texture = create_texture_buffer(&device, display_desc, ENCODER_BUFFER_SIZE).unwrap();

    let ((writer, reader), encoder_params) = encoder_channel(
        &device,
        display_desc,
        &buffer_texture,
        codec,
        preset,
        tuning_info,
    )
    .unwrap();

    let encoder_input = NvidiaEncoder::new(
        writer,
        device_context.unwrap(),
        buffer_texture,
        encoder_params,
    );

    let encoder_output = EncoderOutput::new(reader);

    (encoder_input, encoder_output)
}

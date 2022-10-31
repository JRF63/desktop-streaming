use super::{
    config::EncodeParams,
    device::DeviceImplTrait,
    event::EventObjectTrait,
    raw_encoder::RawEncoder,
    shared::NvidiaEncoderWriter,
    texture::{TextureBufferImplTrait, TextureImplTrait},
};
use crate::Result;
use std::{mem::MaybeUninit, ops::Deref};

pub struct EncoderInput<D: DeviceImplTrait> {
    device: D,
    writer: NvidiaEncoderWriter,
    texture_buffer: <D as DeviceImplTrait>::Buffer,
    encode_pic_params: crate::sys::NV_ENC_PIC_PARAMS,
    encode_params: EncodeParams,
}

impl<D: DeviceImplTrait> Drop for EncoderInput<D> {
    fn drop(&mut self) {
        let _ = self.end_encode();
    }
}

impl<D: DeviceImplTrait> EncoderInput<D> {
    pub(crate) fn new(
        device: D,
        writer: NvidiaEncoderWriter,
        texture_buffer: <D as DeviceImplTrait>::Buffer,
        encode_params: EncodeParams,
    ) -> Result<Self> {
        let encode_pic_params = {
            let mut tmp: crate::sys::NV_ENC_PIC_PARAMS =
                unsafe { MaybeUninit::zeroed().assume_init() };
            tmp.version = crate::sys::NV_ENC_PIC_PARAMS_VER;
            tmp.inputWidth = encode_params.encode_width();
            tmp.inputHeight = encode_params.encode_height();
            tmp.inputPitch = tmp.inputWidth;
            tmp.bufferFmt = texture_buffer
                .get_texture(0)
                .build_register_resource_args(0)?
                .bufferFormat;
            tmp.pictureStruct = crate::sys::NV_ENC_PIC_STRUCT::NV_ENC_PIC_STRUCT_FRAME;
            tmp
        };

        Ok(EncoderInput {
            device,
            writer,
            texture_buffer,
            encode_pic_params,
            encode_params,
        })
    }

    pub fn update_average_bitrate(&mut self, bitrate: u32) -> Result<()> {
        self.encode_params
            .set_average_bitrate(&self.writer, bitrate)
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
        frame: <D as DeviceImplTrait>::Texture,
        timestamp: u64,
        mut post_copy_op: F,
    ) -> Result<()>
    where
        F: FnMut(),
    {
        let pic_params = &mut self.encode_pic_params;
        let device = &self.device;
        let input_textures = &self.texture_buffer;
        let raw_encoder: &RawEncoder = self.writer.deref();

        self.writer.write(frame, |index, buffer, frame| {
            device.copy_texture(input_textures, &frame, index);
            post_copy_op();

            buffer.mapped_input = map_input(raw_encoder, buffer.registered_resource.as_ptr())?;
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
        let pic_params = &mut self.encode_pic_params;

        self.writer.write((), |_, buffer, ()| {
            buffer.end_of_stream = true;
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

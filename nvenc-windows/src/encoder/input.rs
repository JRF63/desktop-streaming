use super::{EncoderParams, NvidiaEncoder, Result};
use crate::nvenc_function;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::{mem::MaybeUninit, sync::Arc};
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
    avail_indices_receiver: Receiver<usize>,
    occupied_indices_sender: Sender<usize>,
}

unsafe impl Send for EncoderInput {}

/// Return type of `EncoderInput::new` instead of returning a tuple.
pub(crate) struct EncoderInputReturn {
    pub(crate) encoder_input: EncoderInput,
    pub(crate) avail_indices_sender: Sender<usize>,
    pub(crate) occupied_indices_receiver: Receiver<usize>,
}

impl EncoderInput {
    pub(crate) fn new(
        encoder: Arc<NvidiaEncoder>,
        device_context: ID3D11DeviceContext,
        encoder_params: EncoderParams,
        buf_size: usize,
    ) -> EncoderInputReturn {
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

        let (avail_indices_sender, avail_indices_receiver) = bounded(buf_size);
        let (occupied_indices_sender, occupied_indices_receiver) = bounded(buf_size);

        for i in 0..buf_size {
            // This should not fail
            avail_indices_sender.send(i).unwrap();
        }

        let encoder_input = EncoderInput {
            encoder,
            device_context,
            pic_params,
            encoder_params,
            avail_indices_receiver,
            occupied_indices_sender,
        };

        EncoderInputReturn {
            encoder_input,
            avail_indices_sender,
            occupied_indices_receiver,
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
            let mut params: nvenc_sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD = MaybeUninit::zeroed().assume_init();
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

    pub fn get_index(&self) -> usize {
        self.avail_indices_receiver.try_recv().unwrap()
    }

    /// Copies the passed resource to the internal texture buffer.
    #[inline]
    pub fn copy_input_frame(&self, frame: IDXGIResource, subresource_index: usize) {
        unsafe {
            // `IDXGIResource` to `ID3D11Texture2D` should never fail
            let acquired_image: ID3D11Texture2D = frame.cast().unwrap_unchecked();
            self.device_context.CopySubresourceRegion(
                &self.encoder.input_textures,
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

    #[inline]
    pub fn encode_copied_frame(&mut self, index: usize) -> Result<()> {
        let input_buf = self.map_input(self.encoder.buffers[index].registered_resource.as_ptr())?;
        unsafe {
            *self.encoder.buffers[index].input_ptr.get() = input_buf;
        }
        self.pic_params.inputBuffer = input_buf;
        self.pic_params.outputBitstream = self.encoder.buffers[index].output_ptr.as_ptr();
        self.pic_params.completionEvent = self.encoder.buffers[index].event_obj.as_ptr();

        unsafe {
            nvenc_function!(
                self.encoder.functions.nvEncEncodePicture,
                self.encoder.raw_encoder.as_ptr(),
                &mut self.pic_params
            );
        }

        // used for invalidation of frames
        self.pic_params.inputTimeStamp += 1;

        Ok(())
    }

    pub fn return_index(&self, index: usize) {
        // TODO: Handle `try_send` error
        self.occupied_indices_sender.try_send(index).unwrap();
    }

    /// Does not seem to function as a sync barrier. Texture copy only syncs on call to
    /// `nvEncEncodePicture` if async encode is enabled.
    #[inline]
    fn map_input(
        &mut self,
        registered_resource: nvenc_sys::NV_ENC_REGISTERED_PTR,
    ) -> Result<nvenc_sys::NV_ENC_INPUT_PTR> {
        let mut map_input_resource_params: nvenc_sys::NV_ENC_MAP_INPUT_RESOURCE =
            unsafe { MaybeUninit::zeroed().assume_init() };
        map_input_resource_params.version = nvenc_sys::NV_ENC_MAP_INPUT_RESOURCE_VER;
        map_input_resource_params.registeredResource = registered_resource;

        unsafe {
            nvenc_function!(
                self.encoder.functions.nvEncMapInputResource,
                self.encoder.raw_encoder.as_ptr(),
                &mut map_input_resource_params
            );
        }
        Ok(map_input_resource_params.mappedResource)
    }
}

use super::{NvidiaEncoder, Result};
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
    encoder_params: nvenc_sys::NV_ENC_RECONFIGURE_PARAMS,
    frame_receiver: Receiver<IDXGIResource>,
    copy_complete_sender: Sender<()>,
    avail_indices_receiver: Receiver<usize>,
    occupied_indices_sender: Sender<usize>,
}

unsafe impl Send for EncoderInput {}

/// Return type of `EncoderInput::new` instead of returning a tuple.
pub(crate) struct EncoderInputReturn {
    pub(crate) encoder_input: EncoderInput,
    pub(crate) frame_sender: Sender<IDXGIResource>,
    pub(crate) copy_complete_receiver: Receiver<()>,
    pub(crate) avail_indices_sender: Sender<usize>,
    pub(crate) occupied_indices_receiver: Receiver<usize>,
}

impl EncoderInput {
    pub(crate) fn new(
        encoder: Arc<NvidiaEncoder>,
        device_context: ID3D11DeviceContext,
        init_params: nvenc_sys::NV_ENC_INITIALIZE_PARAMS,
        buf_size: usize,
    ) -> EncoderInputReturn {
        let pic_params = {
            let mut tmp: nvenc_sys::NV_ENC_PIC_PARAMS =
                unsafe { MaybeUninit::zeroed().assume_init() };
            tmp.version = nvenc_sys::NV_ENC_PIC_PARAMS_VER;
            tmp.inputWidth = init_params.encodeWidth;
            tmp.inputHeight = init_params.encodeHeight;
            tmp.inputPitch = tmp.inputWidth;
            tmp.bufferFmt = init_params.bufferFormat;
            tmp.pictureStruct = nvenc_sys::NV_ENC_PIC_STRUCT::NV_ENC_PIC_STRUCT_FRAME;
            tmp
        };

        let encoder_params = {
            let mut tmp: nvenc_sys::NV_ENC_RECONFIGURE_PARAMS =
                unsafe { MaybeUninit::zeroed().assume_init() };
            tmp.version = nvenc_sys::NV_ENC_RECONFIGURE_PARAMS_VER;
            tmp.reInitEncodeParams = init_params;
            tmp
        };

        let (frame_sender, frame_receiver) = bounded(0);
        let (copy_complete_sender, copy_complete_receiver) = bounded(0);
        let (avail_indices_sender, avail_indices_receiver) = bounded(buf_size);
        let (occupied_indices_sender, occupied_indices_receiver) = bounded(buf_size);

        for i in 0..buf_size {
            avail_indices_sender.send(i).unwrap();
        }

        let encoder_input = EncoderInput {
            encoder,
            device_context,
            pic_params,
            encoder_params,
            frame_receiver,
            copy_complete_sender,
            avail_indices_receiver,
            occupied_indices_sender,
        };

        EncoderInputReturn {
            encoder_input,
            frame_sender,
            copy_complete_receiver,
            avail_indices_sender,
            occupied_indices_receiver,
        }
    }

    pub fn end_encode(&mut self) {
        todo!()
        // self.pic_params.encodePicFlags = nvenc_sys::NV_ENC_PIC_FLAG_EOS;
    }

    pub fn update_pic_params(&mut self) {
        todo!()
    }

    /// Waits for a frame to be encoded the copies it to a texture buffer and encodes it.
    pub fn wait_and_encode_frame(&mut self) -> Result<()> {
        // TODO: Handle `try_recv` error
        let current_index = self.avail_indices_receiver.try_recv().unwrap();

        // TODO: Handle `recv` error
        let frame = self.frame_receiver.recv().unwrap();
        self.copy_input_frame(frame, &self.encoder.input_textures, current_index);

        let input_buf = self.map_input(
            self.encoder.buffers[current_index]
                .registered_resource
                .as_ptr(),
        )?;
        unsafe {
            *self.encoder.buffers[current_index].input_ptr.get() = input_buf;
        }
        self.pic_params.inputBuffer = input_buf;
        self.pic_params.outputBitstream = self.encoder.buffers[current_index].output_ptr.as_ptr();
        self.pic_params.completionEvent = self.encoder.buffers[current_index].event_obj.as_ptr();

        unsafe {
            nvenc_function!(
                self.encoder.functions.nvEncEncodePicture,
                self.encoder.raw_encoder.as_ptr(),
                &mut self.pic_params
            );
        }

        // TODO: Handle `send` error
        self.copy_complete_sender.send(()).unwrap();

        // used for invalidation of frames
        self.pic_params.inputTimeStamp += 1;

        // TODO: Handle `try_send` error
        self.occupied_indices_sender
            .try_send(current_index)
            .unwrap();
        Ok(())
    }

    /// Copies the passed resource to the internal texture buffer.
    #[inline]
    fn copy_input_frame(
        &self,
        frame: IDXGIResource,
        textures: &ID3D11Texture2D,
        current_index: usize,
    ) {
        let acquired_image: ID3D11Texture2D = frame.cast().unwrap();
        unsafe {
            self.device_context.CopySubresourceRegion(
                textures,
                current_index as u32,
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

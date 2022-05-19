use super::{NvidiaEncoder, Result};
use crate::nvenc_function;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::{ffi::c_void, sync::Arc, mem::MaybeUninit};
use windows::{
    core::Interface,
    Win32::Graphics::{
        Direct3D11::{ID3D11DeviceContext, ID3D11Texture2D, D3D11_QUERY_EVENT, D3D11_QUERY_DESC},
        Dxgi::IDXGIResource,
    },
};

pub struct EncoderInput<const BUF_SIZE: usize> {
    encoder: Arc<NvidiaEncoder<BUF_SIZE>>,
    device_context: ID3D11DeviceContext,
    pic_params: nvenc_sys::NV_ENC_PIC_PARAMS,
    encoder_params: nvenc_sys::NV_ENC_RECONFIGURE_PARAMS,
    frame_receiver: Receiver<IDXGIResource>,
    copy_complete_sender: Sender<()>,
    avail_indices_receiver: Receiver<usize>,
    occupied_indices_sender: Sender<usize>,
}

unsafe impl<const BUF_SIZE: usize> Send for EncoderInput<BUF_SIZE> {}

/// Return type of `EncoderInput::new` instead of returning a tuple.
pub(crate) struct EncoderInputReturn<const BUF_SIZE: usize> {
    pub(crate) encoder_input: EncoderInput<BUF_SIZE>,
    pub(crate) frame_sender: Sender<IDXGIResource>,
    pub(crate) copy_complete_receiver: Receiver<()>,
    pub(crate) avail_indices_sender: Sender<usize>,
    pub(crate) occupied_indices_receiver: Receiver<usize>,
}

impl<const BUF_SIZE: usize> EncoderInput<BUF_SIZE> {
    pub(crate) fn new(
        encoder: Arc<NvidiaEncoder<BUF_SIZE>>,
        device_context: ID3D11DeviceContext,
        init_params: nvenc_sys::NV_ENC_INITIALIZE_PARAMS,
    ) -> EncoderInputReturn<BUF_SIZE> {
        let pic_params = {
            let mut tmp: nvenc_sys::NV_ENC_PIC_PARAMS = unsafe { MaybeUninit::zeroed().assume_init() };
            tmp.version = nvenc_sys::NV_ENC_PIC_PARAMS_VER;
            tmp.inputWidth = init_params.encodeWidth;
            tmp.inputHeight = init_params.encodeHeight;
            tmp.inputPitch = tmp.inputWidth;
            tmp.bufferFmt = init_params.bufferFormat;
            tmp.pictureStruct = nvenc_sys::NV_ENC_PIC_STRUCT::NV_ENC_PIC_STRUCT_FRAME;
            tmp
        };

        let encoder_params = {
            let mut tmp: nvenc_sys::NV_ENC_RECONFIGURE_PARAMS = unsafe { MaybeUninit::zeroed().assume_init() };
            tmp.version = nvenc_sys::NV_ENC_RECONFIGURE_PARAMS_VER;
            tmp.reInitEncodeParams = init_params;
            tmp
        };

        let (frame_sender, frame_receiver) = bounded(0);
        let (copy_complete_sender, copy_complete_receiver) = bounded(0);
        let (avail_indices_sender, avail_indices_receiver) = bounded(BUF_SIZE);
        let (occupied_indices_sender, occupied_indices_receiver) = bounded(BUF_SIZE);

        for i in 0..BUF_SIZE {
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
        self.copy_input_frame(frame, &self.encoder.io[current_index].texture);
        // TODO: Handle `send` error
        self.copy_complete_sender.send(()).unwrap();

        let input_buf =
            self.map_input(self.encoder.io[current_index].registered_resource.as_ptr())?;
        unsafe { *self.encoder.io[current_index].input_ptr.get() = input_buf; }
        self.pic_params.inputBuffer = input_buf;
        self.pic_params.outputBitstream = self.encoder.io[current_index].output_ptr.as_ptr();
        self.pic_params.completionEvent = self.encoder.io[current_index].event_obj.0 as *mut c_void;

        // unsafe {
        //     nvenc_function!(
        //         self.encoder.functions.nvEncEncodePicture,
        //         self.encoder.raw_encoder.as_ptr(),
        //         &mut self.pic_params
        //     );
        // }

        // // used for invalidation of frames
        // self.pic_params.inputTimeStamp += 1;

        // // TODO: Handle `try_send` error
        // self.occupied_indices_sender
        //     .try_send(current_index)
        //     .unwrap();
        Ok(())
    }

    /// Copies the passed resource to the internal texture buffer.
    #[inline]
    fn copy_input_frame(&self, frame: IDXGIResource, texture_buffer: &ID3D11Texture2D) {
        let acquired_image: ID3D11Texture2D = frame.cast().unwrap();
        unsafe {
            self.device_context
                .CopyResource(texture_buffer, &acquired_image);
        }
        // self.synchronize_gpu_operation()?;
    }

    // /// GPU operations like CopySubresourceRegion are async and this function
    // /// makes it _absolutely_ sure the texture is copied when the GPU accesses
    // /// its buffer.
    // #[inline(always)]
    // fn synchronize_gpu_operation(&mut self) -> Result<()> {
    //     let mut is_done = BOOL(0);

    //     let copy_done_desc = D3D11_QUERY_DESC {
    //         Query: D3D11_QUERY_EVENT,
    //         MiscFlags: 0,
    //     };

    //     let mut flushed = false;

    //     unsafe {
    //         let query = self.d3d11_device.CreateQuery(&copy_done_desc)?;
    //         self.device_context.End(&query);

    //         loop {
    //             let query_result = self.device_context.GetData(
    //                 &query,
    //                 (&mut is_done as *mut BOOL).cast(),
    //                 std::mem::size_of::<BOOL>() as u32,
    //                 0,
    //             );

    //             if query_result.is_ok() && is_done.as_bool() {
    //                 break;
    //             }

    //             if !flushed {
    //                 self.device_context.Flush();
    //                 flushed = true;
    //             }
    //         }
    //     }

    //     Ok(())
    // }

    /// This acts as a sync barrier - the input texture must not be modified before calling
    /// `nvEncUnmapInputResource` on `EncoderOutput`.
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
            // debug_assert_eq!(mapping.mappedBufferFmt, self.pic_params.bufferFmt);
        }
        Ok(map_input_resource_params.mappedResource)
    }
}

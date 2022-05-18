use crate::{nvenc_function, Result, RawEncoder};
use crossbeam_channel::{Receiver, Sender};
use std::sync::Arc;
use windows::{
    core::Interface,
    Win32::{
        Graphics::{
            Direct3D11::{ID3D11DeviceContext, ID3D11Texture2D},
            Dxgi::IDXGIResource,
        },
        System::{
            Threading::{WaitForSingleObject, WAIT_OBJECT_0},
            WindowsProgramming::INFINITE,
        },
    },
};

pub struct EncoderInput<const BUF_SIZE: usize> {
    encoder: Arc<RawEncoder<BUF_SIZE>>,
    device_context: ID3D11DeviceContext,
    frames_to_encode: Receiver<IDXGIResource>,
    copy_complete: Sender<()>,
    pic_params: nvenc_sys::NV_ENC_PIC_PARAMS,
    avail_indices: Receiver<usize>,
    occupied_indices: Sender<usize>,
}

impl<const BUF_SIZE: usize> EncoderInput<BUF_SIZE> {
    pub fn update_pic_params(&mut self) {}

    /// Waits for a frame to be encoded the copies it to a texture buffer and encodes it.
    pub fn wait_and_encode_frame(&mut self) -> Result<()> {
        // TODO: Handle `try_recv` error
        let current_index = self.avail_indices.try_recv().unwrap();

        // TODO: Handle `recv` error
        let frame = self.frames_to_encode.recv().unwrap();
        self.copy_input_frame(frame, &self.encoder.io[current_index].d3d11_texture);
        // TODO: Handle `send` error
        self.copy_complete.send(()).unwrap();

        let input_buf =
            self.map_input(self.encoder.io[current_index].registered_resource.as_ptr())?;
        self.pic_params.inputBuffer = input_buf;
        self.pic_params.outputBitstream = self.encoder.io[current_index].output_ptr.as_ptr();

        unsafe {
            nvenc_function!(
                self.encoder.functions.nvEncEncodePicture,
                self.encoder.raw_encoder.as_ptr(),
                &mut self.pic_params
            );
        }

        // TODO: Handle `try_send` error
        self.occupied_indices.try_send(current_index).unwrap();
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

    /// This acts as a sync barrier - the input texture must not be modified before calling
    /// `nvEncUnmapInputResource` on `EncoderOutput`.
    #[inline]
    fn map_input(
        &mut self,
        registered_resource: nvenc_sys::NV_ENC_REGISTERED_PTR,
    ) -> Result<nvenc_sys::NV_ENC_INPUT_PTR> {
        let mut map_input_resource_params: nvenc_sys::NV_ENC_MAP_INPUT_RESOURCE =
            unsafe { std::mem::zeroed() };
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

pub struct EncoderOutput<const BUF_SIZE: usize> {
    encoder: Arc<RawEncoder<BUF_SIZE>>,
    occupied_indices: Receiver<usize>,
    avail_indices: Sender<usize>,
}

impl<const BUF_SIZE: usize> EncoderOutput<BUF_SIZE> {
    pub fn wait_for_output<F: FnMut(&nvenc_sys::NV_ENC_LOCK_BITSTREAM) -> ()>(
        &self,
        mut consume_output: F,
    ) -> Result<()> {
        // TODO: Handle `recv` error
        let index = self.occupied_indices.recv().unwrap();
        match unsafe { WaitForSingleObject(self.encoder.io[index].event_obj, INFINITE) } {
            WAIT_OBJECT_0 => {
                let mut lock_params: nvenc_sys::NV_ENC_LOCK_BITSTREAM =
                    unsafe { std::mem::zeroed() };
                lock_params.version = nvenc_sys::NV_ENC_LOCK_BITSTREAM_VER;
                lock_params.outputBitstream = self.encoder.io[index].output_ptr.as_ptr();

                unsafe {
                    nvenc_function!(
                        self.encoder.functions.nvEncLockBitstream,
                        self.encoder.raw_encoder.as_ptr(),
                        &mut lock_params
                    );
                }

                consume_output(&lock_params);

                unsafe {
                    nvenc_function!(
                        self.encoder.functions.nvEncUnlockBitstream,
                        self.encoder.raw_encoder.as_ptr(),
                        lock_params.outputBitstream
                    );

                    nvenc_function!(
                        self.encoder.functions.nvEncUnmapInputResource,
                        self.encoder.raw_encoder.as_ptr(),
                        self.encoder.io[index].input_ptr
                    );
                }
            }
            _ => panic!("Waiting for event object failed."), // TODO: Handle error
        }
        // TODO: Handle `try_send` error
        self.avail_indices.try_send(index).unwrap();
        Ok(())
    }
}

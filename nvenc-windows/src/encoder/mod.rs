mod buffer;
mod config;
mod init;
mod output;
mod queries;
mod shared;
mod raw;

use self::init::*;
use crate::{nvenc_function, sync::CyclicBuffer, Codec, EncoderPreset, Result, TuningInfo};
use buffer::NvidiaEncoderBufferItems;
use config::EncoderParams;
use output::EncoderOutput;
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull, sync::Arc};
use windows::{
    core::Interface,
    Win32::Graphics::{
        Direct3D11::{ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D},
        Dxgi::{IDXGIResource, DXGI_OUTDUPL_DESC},
    },
};

use crate::os::windows::{create_texture_buffer, Library};

pub(crate) struct NvidiaEncoderShared<const BUF_SIZE: usize> {
    raw_encoder: NonNull<c_void>,
    functions: nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    buffer: CyclicBuffer<NvidiaEncoderBufferItems, BUF_SIZE>,

    #[allow(dead_code)]
    library: Library,
}

impl<const BUF_SIZE: usize> Drop for NvidiaEncoderShared<BUF_SIZE> {
    fn drop(&mut self) {
        for buffer in self.buffer.get_mut() {
            buffer.get_mut().cleanup(&self.functions, self.raw_encoder);
        }
        unsafe {
            (self.functions.nvEncDestroyEncoder.unwrap())(self.raw_encoder.as_ptr());
        }
    }
}

// TODO: `Sync` and `Send` are technically wrong
unsafe impl<const BUF_SIZE: usize> Sync for NvidiaEncoderShared<BUF_SIZE> {}
unsafe impl<const BUF_SIZE: usize> Send for NvidiaEncoderShared<BUF_SIZE> {}

impl<const BUF_SIZE: usize> NvidiaEncoderShared<BUF_SIZE> {
    pub(crate) fn new(
        device: ID3D11Device,
        display_desc: &DXGI_OUTDUPL_DESC,
        buffer_texture: &ID3D11Texture2D,
        codec: Codec,
        preset: EncoderPreset,
        tuning_info: TuningInfo,
    ) -> anyhow::Result<(Self, EncoderParams)> {
        assert!(BUF_SIZE.count_ones() == 1, "Buffer size must be a power of two");

        let library = Library::load("nvEncodeAPI64.dll")?;
        if !is_version_supported(&library)? {
            return Err(anyhow::anyhow!(
                "NVENC version is not supported by the installed driver"
            ));
        }
        let functions = get_function_list(&library)?;
        let raw_encoder = open_encode_session(&functions, &device)?;

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
                encoder_params.init_params_mut()
            );
        }

        let buffer = unsafe {
            let mut buffer = MaybeUninit::<[NvidiaEncoderBufferItems; BUF_SIZE]>::uninit();

            // Pointer to the start of the array's buffer
            let mut ptr = (&mut *buffer.as_mut_ptr()).as_mut_ptr();

            for i in 0..BUF_SIZE {
                ptr.write(NvidiaEncoderBufferItems::new(
                    &functions,
                    raw_encoder,
                    buffer_texture,
                    i as u32,
                )?);
                ptr = ptr.offset(1);
            }
            buffer.assume_init()
        };

        Ok((
            NvidiaEncoderShared {
                raw_encoder,
                functions,
                buffer: CyclicBuffer::new(buffer),
                library,
            },
            encoder_params,
        ))
    }
}

pub struct NvidiaEncoder<const BUF_SIZE: usize> {
    shared: Arc<NvidiaEncoderShared<BUF_SIZE>>,
    device_context: ID3D11DeviceContext,
    buffer_texture: ID3D11Texture2D,
    pic_params: nvenc_sys::NV_ENC_PIC_PARAMS,
    encoder_params: EncoderParams,
}

impl<const BUF_SIZE: usize> NvidiaEncoder<BUF_SIZE> {
    pub(crate) fn new(
        shared: Arc<NvidiaEncoderShared<BUF_SIZE>>,
        device_context: ID3D11DeviceContext,
        buffer_texture: ID3D11Texture2D,
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

        NvidiaEncoder {
            shared,
            device_context,
            buffer_texture,
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
                self.shared.functions.nvEncReconfigureEncoder,
                self.shared.raw_encoder.as_ptr(),
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
                self.shared.functions.nvEncGetSequenceParams,
                self.shared.raw_encoder.as_ptr(),
                &mut params
            );

            buffer.truncate(bytes_written as usize);
            Ok(buffer)
        }
    }

    pub fn encode_frame(&mut self, frame: IDXGIResource, timestamp: u32) -> Result<()> {
        let pic_params = &mut self.pic_params;
        let device_context = &self.device_context;
        let input_textures = &self.buffer_texture;
        let functions = &self.shared.functions;
        let raw_encoder = self.shared.raw_encoder;

        self.shared.buffer.writer_access(|index, buffer| {
            NvidiaEncoder::<BUF_SIZE>::copy_input_frame(device_context, input_textures, &frame, index);

            buffer.mapped_input = NvidiaEncoder::<BUF_SIZE>::map_input(
                functions,
                raw_encoder,
                buffer.registered_resource.as_ptr(),
            )
            .unwrap();
            pic_params.inputBuffer = buffer.mapped_input;
            pic_params.outputBitstream = buffer.output_buffer.as_ptr();
            pic_params.completionEvent = buffer.event_obj.as_ptr();
        });
        // Already copied
        std::mem::drop(frame);

        // Used for invalidation of frames
        self.pic_params.inputTimeStamp = timestamp as u64;

        unsafe {
            nvenc_function!(
                self.shared.functions.nvEncEncodePicture,
                self.shared.raw_encoder.as_ptr(),
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

pub fn create_encoder<const BUF_SIZE: usize>(
    device: ID3D11Device,
    display_desc: &DXGI_OUTDUPL_DESC,
    codec: Codec,
    preset: EncoderPreset,
    tuning_info: TuningInfo,
) -> (NvidiaEncoder<BUF_SIZE>, EncoderOutput<BUF_SIZE>) {
    let mut device_context = None;
    unsafe {
        device.GetImmediateContext(&mut device_context);
    }

    let buffer_texture = create_texture_buffer(&device, display_desc, BUF_SIZE).unwrap();

    let (encoder, encoder_params) = NvidiaEncoderShared::new(
        device,
        display_desc,
        &buffer_texture,
        codec,
        preset,
        tuning_info,
    )
    .unwrap();
    let encoder = Arc::new(encoder);

    let encoder_input = NvidiaEncoder::new(
        encoder.clone(),
        device_context.unwrap(),
        buffer_texture,
        encoder_params,
    );

    let encoder_output = EncoderOutput::new(encoder);

    (encoder_input, encoder_output)
}

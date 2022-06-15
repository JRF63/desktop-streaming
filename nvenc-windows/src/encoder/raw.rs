use crate::{util::NvEncDevice, NvEncError, Result};
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull};

/// Start an encoding session.
fn open_encode_session<T: NvEncDevice>(
    functions: &nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
    device: &T,
) -> Result<NonNull<c_void>> {
    let mut raw_encoder: *mut c_void = std::ptr::null_mut();
    unsafe {
        let mut session_params: nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS =
            MaybeUninit::zeroed().assume_init();
        session_params.version = nvenc_sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
        session_params.deviceType = T::device_type();
        session_params.device = device.as_ptr();
        session_params.apiVersion = nvenc_sys::NVENCAPI_VERSION;

        let status =
            (functions.nvEncOpenEncodeSessionEx.unwrap())(&mut session_params, &mut raw_encoder);
        match NvEncError::from_nvenc_status(status) {
            // Should not fail if `nvEncOpenEncodeSessionEx` succeeded
            None => NonNull::new(raw_encoder).ok_or(NvEncError::Generic),
            Some(err) => Err(err),
        }
    }
}

pub(crate) struct RawEncoder {
    encoder_ptr: NonNull<c_void>,
    functions: nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
}

unsafe impl Send for RawEncoder {}

impl Drop for RawEncoder {
    fn drop(&mut self) {
        unsafe {
            let _status =
                (self.functions.nvEncDestroyEncoder.unwrap_unchecked())(self.encoder_ptr.as_ptr());
        }
    }
}

impl RawEncoder {
    pub(crate) fn new<T: NvEncDevice>(
        functions: nvenc_sys::NV_ENCODE_API_FUNCTION_LIST,
        device: &T,
    ) -> Result<Self> {
        Ok(RawEncoder {
            encoder_ptr: open_encode_session(&functions, device)?,
            functions,
        })
    }
    pub(crate) fn get_encode_guid_count(&self, encode_guid_count: *mut u32) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetEncodeGUIDCount.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_guid_count,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_encode_guids(
        &self,
        guids: *mut nvenc_sys::GUID,
        guid_array_size: u32,
        guid_count: *mut u32,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetEncodeGUIDs.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                guids,
                guid_array_size,
                guid_count,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_encode_preset_count(
        &self,
        encode_guid: nvenc_sys::GUID,
        encode_profile_guid_count: *mut u32,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetEncodePresetCount.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_guid,
                encode_profile_guid_count,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_encode_preset_guids(
        &self,
        encode_guid: nvenc_sys::GUID,
        profile_guids: *mut nvenc_sys::GUID,
        guid_array_size: u32,
        guid_count: *mut u32,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetEncodePresetGUIDs.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_guid,
                profile_guids,
                guid_array_size,
                guid_count,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_input_format_count(
        &self,
        encode_guid: nvenc_sys::GUID,
        input_fmt_count: *mut u32,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetInputFormatCount.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_guid,
                input_fmt_count,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_input_formats(
        &self,
        encode_guid: nvenc_sys::GUID,
        input_fmts: *mut nvenc_sys::NV_ENC_BUFFER_FORMAT,
        input_fmt_array_size: u32,
        input_fmt_count: *mut u32,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetInputFormats.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_guid,
                input_fmts,
                input_fmt_array_size,
                input_fmt_count,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_encode_caps(
        &self,
        encode_guid: nvenc_sys::GUID,
        caps_param: *mut nvenc_sys::NV_ENC_CAPS_PARAM,
        caps_val: *mut ::std::os::raw::c_int,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetEncodeCaps.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_guid,
                caps_param,
                caps_val,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_encode_preset_config(
        &self,
        encode_guid: nvenc_sys::GUID,
        preset_guid: nvenc_sys::GUID,
        preset_config: *mut nvenc_sys::NV_ENC_PRESET_CONFIG,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetEncodePresetConfig.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_guid,
                preset_guid,
                preset_config,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_encode_preset_config_ex(
        &self,
        encode_guid: nvenc_sys::GUID,
        preset_guid: nvenc_sys::GUID,
        tuning_info: nvenc_sys::NV_ENC_TUNING_INFO,
        preset_config: *mut nvenc_sys::NV_ENC_PRESET_CONFIG,
    ) -> Result<()> {
        unsafe {
            let status = (self
                .functions
                .nvEncGetEncodePresetConfigEx
                .unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_guid,
                preset_guid,
                tuning_info,
                preset_config,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn initialize_encoder(
        &self,
        create_encode_params: *mut nvenc_sys::NV_ENC_INITIALIZE_PARAMS,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncInitializeEncoder.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                create_encode_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn create_input_buffer(
        &self,
        create_input_buffer_params: *mut nvenc_sys::NV_ENC_CREATE_INPUT_BUFFER,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncCreateInputBuffer.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                create_input_buffer_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn destroy_input_buffer(
        &self,
        input_buffer: nvenc_sys::NV_ENC_INPUT_PTR,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncDestroyInputBuffer.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                input_buffer,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn create_bitstream_buffer(
        &self,
        create_bitstream_buffer_params: *mut nvenc_sys::NV_ENC_CREATE_BITSTREAM_BUFFER,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncCreateBitstreamBuffer.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                create_bitstream_buffer_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn destroy_bitstream_buffer(
        &self,
        bitstream_buffer: nvenc_sys::NV_ENC_OUTPUT_PTR,
    ) -> Result<()> {
        unsafe {
            let status = (self
                .functions
                .nvEncDestroyBitstreamBuffer
                .unwrap_unchecked())(
                self.encoder_ptr.as_ptr(), bitstream_buffer
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn encode_picture(
        &self,
        encode_pic_params: *mut nvenc_sys::NV_ENC_PIC_PARAMS,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncEncodePicture.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_pic_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn lock_bitstream(
        &self,
        lock_bitstream_buffer_params: *mut nvenc_sys::NV_ENC_LOCK_BITSTREAM,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncLockBitstream.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                lock_bitstream_buffer_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn unlock_bitstream(
        &self,
        bitstream_buffer: nvenc_sys::NV_ENC_OUTPUT_PTR,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncUnlockBitstream.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                bitstream_buffer,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn lock_input_buffer(
        &self,
        lock_input_buffer_params: *mut nvenc_sys::NV_ENC_LOCK_INPUT_BUFFER,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncLockInputBuffer.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                lock_input_buffer_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn unlock_input_buffer(
        &self,
        input_buffer: nvenc_sys::NV_ENC_INPUT_PTR,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncUnlockInputBuffer.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                input_buffer,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_encode_stats(&self, encode_stats: *mut nvenc_sys::NV_ENC_STAT) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetEncodeStats.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                encode_stats,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_sequence_params(
        &self,
        sequence_param_payload: *mut nvenc_sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetSequenceParams.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                sequence_param_payload,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn register_async_event(
        &self,
        event_params: *mut nvenc_sys::NV_ENC_EVENT_PARAMS,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncRegisterAsyncEvent.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                event_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn unregister_async_event(
        &self,
        event_params: *mut nvenc_sys::NV_ENC_EVENT_PARAMS,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncUnregisterAsyncEvent.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                event_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn map_input_resource(
        &self,
        map_input_res_params: *mut nvenc_sys::NV_ENC_MAP_INPUT_RESOURCE,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncMapInputResource.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                map_input_res_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn unmap_input_resource(
        &self,
        mapped_input_buffer: nvenc_sys::NV_ENC_INPUT_PTR,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncUnmapInputResource.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                mapped_input_buffer,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn invalidate_ref_frames(&self, invalid_ref_frame_time_stamp: u64) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncInvalidateRefFrames.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                invalid_ref_frame_time_stamp,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn register_resource(
        &self,
        register_res_params: *mut nvenc_sys::NV_ENC_REGISTER_RESOURCE,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncRegisterResource.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                register_res_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn unregister_resource(
        &self,
        registered_res: nvenc_sys::NV_ENC_REGISTERED_PTR,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncUnregisterResource.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                registered_res,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn reconfigure_encoder(
        &self,
        re_init_encode_params: *mut nvenc_sys::NV_ENC_RECONFIGURE_PARAMS,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncReconfigureEncoder.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                re_init_encode_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn create_buffer(
        &self,
        create_buffer_params: *mut nvenc_sys::NV_ENC_CREATE_MV_BUFFER,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncCreateMVBuffer.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                create_buffer_params,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn destroy_buffer(&self, mv_buffer: nvenc_sys::NV_ENC_OUTPUT_PTR) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncDestroyMVBuffer.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                mv_buffer,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn run_motion_estimation_only(
        &self,
        me_only_params: *mut nvenc_sys::NV_ENC_MEONLY_PARAMS,
    ) -> Result<()> {
        unsafe {
            let status =
                (self
                    .functions
                    .nvEncRunMotionEstimationOnly
                    .unwrap_unchecked())(self.encoder_ptr.as_ptr(), me_only_params);
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn set_cuda_streams(
        &self,
        input_stream: nvenc_sys::NV_ENC_CUSTREAM_PTR,
        output_stream: nvenc_sys::NV_ENC_CUSTREAM_PTR,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncSetIOCudaStreams.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                input_stream,
                output_stream,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
    pub(crate) fn get_sequence_param_ex(
        &self,
        enc_init_params: *mut nvenc_sys::NV_ENC_INITIALIZE_PARAMS,
        sequence_param_payload: *mut nvenc_sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD,
    ) -> Result<()> {
        unsafe {
            let status = (self.functions.nvEncGetSequenceParamEx.unwrap_unchecked())(
                self.encoder_ptr.as_ptr(),
                enc_init_params,
                sequence_param_payload,
            );
            match NvEncError::from_nvenc_status(status) {
                None => Ok(()),
                Some(err) => Err(err),
            }
        }
    }
}

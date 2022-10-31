#![allow(dead_code)]

use super::{device::DeviceImplTrait, library::Library};
use crate::{NvEncError, Result};
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull};

/// Start an encoding session.
fn open_encode_session<T: DeviceImplTrait>(
    functions: &crate::sys::NV_ENCODE_API_FUNCTION_LIST,
    device: &T,
) -> Result<NonNull<c_void>> {
    let mut raw_encoder: *mut c_void = std::ptr::null_mut();
    unsafe {
        let mut session_params: crate::sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS =
            MaybeUninit::zeroed().assume_init();
        session_params.version = crate::sys::NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
        session_params.deviceType = T::device_type();
        session_params.device = device.as_ptr();
        session_params.apiVersion = crate::sys::NVENCAPI_VERSION;

        let status = (functions.nvEncOpenEncodeSessionEx.unwrap_unchecked())(
            &mut session_params,
            &mut raw_encoder,
        );

        // Should not fail if `nvEncOpenEncodeSessionEx` succeeded
        match NvEncError::from_nvenc_status(status) {
            None => NonNull::new(raw_encoder).ok_or(NvEncError::default()),
            Some(err) => Err(err),
        }
    }
}

/// Checks the function list for null pointers. They all need to be valid since they are going to
/// be `unwrap_unchecked` later.
fn is_function_list_valid(functions: &crate::sys::NV_ENCODE_API_FUNCTION_LIST) -> bool {
    // It could also be transmuted to a &[u8; _] and checked for zeroes that way
    let helper = || -> Option<()> {
        functions.nvEncOpenEncodeSession?;
        functions.nvEncGetEncodeGUIDCount?;
        functions.nvEncGetEncodeProfileGUIDCount?;
        functions.nvEncGetEncodeProfileGUIDs?;
        functions.nvEncGetEncodeGUIDs?;
        functions.nvEncGetInputFormatCount?;
        functions.nvEncGetInputFormats?;
        functions.nvEncGetEncodeCaps?;
        functions.nvEncGetEncodePresetCount?;
        functions.nvEncGetEncodePresetGUIDs?;
        functions.nvEncGetEncodePresetConfig?;
        functions.nvEncInitializeEncoder?;
        functions.nvEncCreateInputBuffer?;
        functions.nvEncDestroyInputBuffer?;
        functions.nvEncCreateBitstreamBuffer?;
        functions.nvEncDestroyBitstreamBuffer?;
        functions.nvEncEncodePicture?;
        functions.nvEncLockBitstream?;
        functions.nvEncUnlockBitstream?;
        functions.nvEncLockInputBuffer?;
        functions.nvEncUnlockInputBuffer?;
        functions.nvEncGetEncodeStats?;
        functions.nvEncGetSequenceParams?;
        functions.nvEncRegisterAsyncEvent?;
        functions.nvEncUnregisterAsyncEvent?;
        functions.nvEncMapInputResource?;
        functions.nvEncUnmapInputResource?;
        functions.nvEncDestroyEncoder?;
        functions.nvEncInvalidateRefFrames?;
        functions.nvEncOpenEncodeSessionEx?;
        functions.nvEncRegisterResource?;
        functions.nvEncUnregisterResource?;
        functions.nvEncReconfigureEncoder?;
        functions.nvEncCreateMVBuffer?;
        functions.nvEncDestroyMVBuffer?;
        functions.nvEncRunMotionEstimationOnly?;
        functions.nvEncGetLastErrorString?;
        functions.nvEncSetIOCudaStreams?;
        functions.nvEncGetEncodePresetConfigEx?;
        functions.nvEncGetSequenceParamEx?;
        Some(())
    };
    helper().is_some()
}

pub struct RawEncoder {
    encoder_ptr: NonNull<c_void>,
    functions: crate::sys::NV_ENCODE_API_FUNCTION_LIST,
    library: Library,
}

// SAFETY: The struct members would not be invalidated by being moved to another thread.
unsafe impl Send for RawEncoder {}

// SAFETY: NvEnc API can handle being called from multiple threads.
unsafe impl Sync for RawEncoder {}

impl Drop for RawEncoder {
    fn drop(&mut self) {
        unsafe {
            let _ =
                (self.functions.nvEncDestroyEncoder.unwrap_unchecked())(self.encoder_ptr.as_ptr());
        }
    }
}

impl RawEncoder {
    pub fn new<T: DeviceImplTrait>(device: &T, library: Library) -> Result<Self> {
        let functions = library.get_function_list()?;
        if !is_function_list_valid(&functions) {
            return Err(NvEncError::MalformedFunctionList);
        }

        Ok(RawEncoder {
            encoder_ptr: open_encode_session(&functions, device)?,
            functions,
            library,
        })
    }
    #[inline]
    pub unsafe fn get_encode_guid_count(&self, encode_guid_count: *mut u32) -> Result<()> {
        let status = (self.functions.nvEncGetEncodeGUIDCount.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            encode_guid_count,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn get_encode_guids(
        &self,
        guids: *mut crate::sys::GUID,
        guid_array_size: u32,
        guid_count: *mut u32,
    ) -> Result<()> {
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
    #[inline]
    pub unsafe fn get_encode_profile_guid_count(
        &self,
        encode_guid: crate::sys::GUID,
        encode_profile_guid_count: *mut u32,
    ) -> Result<()> {
        let status = (self
            .functions
            .nvEncGetEncodeProfileGUIDCount
            .unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            encode_guid,
            encode_profile_guid_count,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn get_encode_profile_guids(
        &self,
        encode_guid: crate::sys::GUID,
        profile_guids: *mut crate::sys::GUID,
        guid_array_size: u32,
        guid_count: *mut u32,
    ) -> Result<()> {
        let status = (self.functions.nvEncGetEncodeProfileGUIDs.unwrap_unchecked())(
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
    #[inline]
    pub unsafe fn get_input_format_count(
        &self,
        encode_guid: crate::sys::GUID,
        input_fmt_count: *mut u32,
    ) -> Result<()> {
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
    #[inline]
    pub unsafe fn get_input_formats(
        &self,
        encode_guid: crate::sys::GUID,
        input_fmts: *mut crate::sys::NV_ENC_BUFFER_FORMAT,
        input_fmt_array_size: u32,
        input_fmt_count: *mut u32,
    ) -> Result<()> {
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
    #[inline]
    pub unsafe fn get_encode_caps(
        &self,
        encode_guid: crate::sys::GUID,
        caps_param: *mut crate::sys::NV_ENC_CAPS_PARAM,
        caps_val: *mut ::std::os::raw::c_int,
    ) -> Result<()> {
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
    #[inline]
    pub unsafe fn get_encode_preset_count(
        &self,
        encode_guid: crate::sys::GUID,
        encode_preset_guid_count: *mut u32,
    ) -> Result<()> {
        let status = (self.functions.nvEncGetEncodePresetCount.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            encode_guid,
            encode_preset_guid_count,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn get_encode_preset_guids(
        &self,
        encode_guid: crate::sys::GUID,
        preset_guids: *mut crate::sys::GUID,
        guid_array_size: u32,
        encode_preset_guid_count: *mut u32,
    ) -> Result<()> {
        let status = (self.functions.nvEncGetEncodePresetGUIDs.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            encode_guid,
            preset_guids,
            guid_array_size,
            encode_preset_guid_count,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn get_encode_preset_config(
        &self,
        encode_guid: crate::sys::GUID,
        preset_guid: crate::sys::GUID,
        preset_config: *mut crate::sys::NV_ENC_PRESET_CONFIG,
    ) -> Result<()> {
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
    #[inline]
    pub unsafe fn get_encode_preset_config_ex(
        &self,
        encode_guid: crate::sys::GUID,
        preset_guid: crate::sys::GUID,
        tuning_info: crate::sys::NV_ENC_TUNING_INFO,
        preset_config: *mut crate::sys::NV_ENC_PRESET_CONFIG,
    ) -> Result<()> {
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
    #[inline]
    pub unsafe fn initialize_encoder(
        &self,
        create_encode_params: *mut crate::sys::NV_ENC_INITIALIZE_PARAMS,
    ) -> Result<()> {
        let status = (self.functions.nvEncInitializeEncoder.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            create_encode_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn create_input_buffer(
        &self,
        create_input_buffer_params: *mut crate::sys::NV_ENC_CREATE_INPUT_BUFFER,
    ) -> Result<()> {
        let status = (self.functions.nvEncCreateInputBuffer.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            create_input_buffer_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn destroy_input_buffer(
        &self,
        input_buffer: crate::sys::NV_ENC_INPUT_PTR,
    ) -> Result<()> {
        let status = (self.functions.nvEncDestroyInputBuffer.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            input_buffer,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn create_bitstream_buffer(
        &self,
        create_bitstream_buffer_params: *mut crate::sys::NV_ENC_CREATE_BITSTREAM_BUFFER,
    ) -> Result<()> {
        let status = (self.functions.nvEncCreateBitstreamBuffer.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            create_bitstream_buffer_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn destroy_bitstream_buffer(
        &self,
        bitstream_buffer: crate::sys::NV_ENC_OUTPUT_PTR,
    ) -> Result<()> {
        let status = (self
            .functions
            .nvEncDestroyBitstreamBuffer
            .unwrap_unchecked())(self.encoder_ptr.as_ptr(), bitstream_buffer);
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn encode_picture(
        &self,
        encode_pic_params: *mut crate::sys::NV_ENC_PIC_PARAMS,
    ) -> Result<()> {
        let status = (self.functions.nvEncEncodePicture.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            encode_pic_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn lock_bitstream(
        &self,
        lock_bitstream_buffer_params: *mut crate::sys::NV_ENC_LOCK_BITSTREAM,
    ) -> Result<()> {
        let status = (self.functions.nvEncLockBitstream.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            lock_bitstream_buffer_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn unlock_bitstream(
        &self,
        bitstream_buffer: crate::sys::NV_ENC_OUTPUT_PTR,
    ) -> Result<()> {
        let status = (self.functions.nvEncUnlockBitstream.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            bitstream_buffer,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn lock_input_buffer(
        &self,
        lock_input_buffer_params: *mut crate::sys::NV_ENC_LOCK_INPUT_BUFFER,
    ) -> Result<()> {
        let status = (self.functions.nvEncLockInputBuffer.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            lock_input_buffer_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn unlock_input_buffer(
        &self,
        input_buffer: crate::sys::NV_ENC_INPUT_PTR,
    ) -> Result<()> {
        let status = (self.functions.nvEncUnlockInputBuffer.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            input_buffer,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn get_encode_stats(
        &self,
        encode_stats: *mut crate::sys::NV_ENC_STAT,
    ) -> Result<()> {
        let status = (self.functions.nvEncGetEncodeStats.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            encode_stats,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn get_sequence_params(
        &self,
        sequence_param_payload: *mut crate::sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD,
    ) -> Result<()> {
        let status = (self.functions.nvEncGetSequenceParams.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            sequence_param_payload,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn register_async_event(
        &self,
        event_params: *mut crate::sys::NV_ENC_EVENT_PARAMS,
    ) -> Result<()> {
        let status = (self.functions.nvEncRegisterAsyncEvent.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            event_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn unregister_async_event(
        &self,
        event_params: *mut crate::sys::NV_ENC_EVENT_PARAMS,
    ) -> Result<()> {
        let status = (self.functions.nvEncUnregisterAsyncEvent.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            event_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn map_input_resource(
        &self,
        map_input_res_params: *mut crate::sys::NV_ENC_MAP_INPUT_RESOURCE,
    ) -> Result<()> {
        let status = (self.functions.nvEncMapInputResource.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            map_input_res_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn unmap_input_resource(
        &self,
        mapped_input_buffer: crate::sys::NV_ENC_INPUT_PTR,
    ) -> Result<()> {
        let status = (self.functions.nvEncUnmapInputResource.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            mapped_input_buffer,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn invalidate_ref_frames(&self, invalid_ref_frame_time_stamp: u64) -> Result<()> {
        let status = (self.functions.nvEncInvalidateRefFrames.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            invalid_ref_frame_time_stamp,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn register_resource(
        &self,
        register_res_params: *mut crate::sys::NV_ENC_REGISTER_RESOURCE,
    ) -> Result<()> {
        let status = (self.functions.nvEncRegisterResource.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            register_res_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn unregister_resource(
        &self,
        registered_res: crate::sys::NV_ENC_REGISTERED_PTR,
    ) -> Result<()> {
        let status = (self.functions.nvEncUnregisterResource.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            registered_res,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn reconfigure_encoder(
        &self,
        re_init_encode_params: *mut crate::sys::NV_ENC_RECONFIGURE_PARAMS,
    ) -> Result<()> {
        let status = (self.functions.nvEncReconfigureEncoder.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            re_init_encode_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn create_buffer(
        &self,
        create_buffer_params: *mut crate::sys::NV_ENC_CREATE_MV_BUFFER,
    ) -> Result<()> {
        let status = (self.functions.nvEncCreateMVBuffer.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            create_buffer_params,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn destroy_buffer(&self, mv_buffer: crate::sys::NV_ENC_OUTPUT_PTR) -> Result<()> {
        let status = (self.functions.nvEncDestroyMVBuffer.unwrap_unchecked())(
            self.encoder_ptr.as_ptr(),
            mv_buffer,
        );
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn run_motion_estimation_only(
        &self,
        me_only_params: *mut crate::sys::NV_ENC_MEONLY_PARAMS,
    ) -> Result<()> {
        let status = (self
            .functions
            .nvEncRunMotionEstimationOnly
            .unwrap_unchecked())(self.encoder_ptr.as_ptr(), me_only_params);
        match NvEncError::from_nvenc_status(status) {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
    #[inline]
    pub unsafe fn set_cuda_streams(
        &self,
        input_stream: crate::sys::NV_ENC_CUSTREAM_PTR,
        output_stream: crate::sys::NV_ENC_CUSTREAM_PTR,
    ) -> Result<()> {
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
    #[inline]
    pub unsafe fn get_sequence_param_ex(
        &self,
        enc_init_params: *mut crate::sys::NV_ENC_INITIALIZE_PARAMS,
        sequence_param_payload: *mut crate::sys::NV_ENC_SEQUENCE_PARAM_PAYLOAD,
    ) -> Result<()> {
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

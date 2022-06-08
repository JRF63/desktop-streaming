use super::{
    format::{MediaFormat, VideoType},
    status::{AsMediaStatus, MediaStatus},
};
use ndk_sys::{
    AMediaCodec, AMediaCodecBufferInfo, AMediaCodec_configure, AMediaCodec_createDecoderByType,
    AMediaCodec_delete, AMediaCodec_dequeueInputBuffer, AMediaCodec_dequeueOutputBuffer,
    AMediaCodec_getInputBuffer, AMediaCodec_queueInputBuffer, AMediaCodec_releaseOutputBuffer,
    AMediaCodec_setOutputSurface, AMediaCodec_start, AMediaCodec_stop, ANativeWindow,
};
use std::{
    os::raw::{c_long, c_ulong},
    ptr::NonNull,
};
use crate::window::NativeWindow;

#[repr(transparent)]
pub(crate) struct MediaCodec(NonNull<AMediaCodec>);

impl Drop for MediaCodec {
    fn drop(&mut self) {
        unsafe {
            let _ignored_result = self.stop();
            AMediaCodec_delete(self.0.as_ptr());
        }
    }
}

impl MediaCodec {
    pub(crate) fn create_video_decoder(
        window: &NativeWindow,
        video_type: VideoType,
        width: i32,
        height: i32,
        frame_rate: i32,
        csd: &[u8],
    ) -> anyhow::Result<Self> {
        let format = MediaFormat::create_video_format(video_type, width, height, frame_rate, csd)?;
        let mut decoder = {
            let ptr = unsafe { AMediaCodec_createDecoderByType(format.get_mime_type()) };
            if let Some(decoder) = NonNull::new(ptr) {
                MediaCodec(decoder)
            } else {
                anyhow::bail!("`NonAMediaCodec_createDecoderByType` returned a null");
            }
        };

        decoder.configure(&format, window)?;
        decoder.read_output_format();
        decoder.start()?;
        Ok(decoder)
    }

    fn read_output_format(&self) {
        use ndk_sys::{
            AMediaCodec_getOutputFormat, AMediaFormat_delete, AMediaFormat_getInt32,
            AMEDIAFORMAT_KEY_BIT_RATE, AMEDIAFORMAT_KEY_HEIGHT, AMEDIAFORMAT_KEY_MAX_BIT_RATE,
            AMEDIAFORMAT_KEY_MAX_HEIGHT, AMEDIAFORMAT_KEY_MAX_WIDTH, AMEDIAFORMAT_KEY_TILE_HEIGHT,
            AMEDIAFORMAT_KEY_TILE_WIDTH, AMEDIAFORMAT_KEY_WIDTH,
        };

        unsafe {
            let format = AMediaCodec_getOutputFormat(self.as_inner());
            if !format.is_null() {
                crate::info!("-------- output format");
                let mut x = 0;
                if AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_BIT_RATE, &mut x) {
                    crate::info!("  bit rate: {}", x);
                }
                if AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_MAX_BIT_RATE, &mut x) {
                    crate::info!("  max bit rate: {}", x);
                }
                if AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_WIDTH, &mut x) {
                    crate::info!("  width: {}", x);
                }
                if AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_HEIGHT, &mut x) {
                    crate::info!("  height: {}", x);
                }
                if AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_MAX_WIDTH, &mut x) {
                    crate::info!("  max width: {}", x);
                }
                if AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_MAX_HEIGHT, &mut x) {
                    crate::info!("  max height: {}", x);
                }
                if AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_TILE_WIDTH, &mut x) {
                    crate::info!("  tile width: {}", x);
                }
                if AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_TILE_HEIGHT, &mut x) {
                    crate::info!("  tile height: {}", x);
                }

                // TODO: Profiles and levels

                AMediaFormat_delete(format);
            }
        }
    }

    pub(crate) fn as_inner(&self) -> *mut AMediaCodec {
        self.0.as_ptr()
    }

    pub(crate) fn set_output_surface(
        &self,
        window: &NativeWindow,
    ) -> Result<(), MediaStatus> {
        unsafe {
            AMediaCodec_setOutputSurface(self.as_inner(), window.as_inner()).success()
        }
    }

    pub(crate) fn try_decode(
        &self,
        data: &[u8],
        time: u64,
        end_of_stream: bool,
    ) -> anyhow::Result<bool> {
        match self.dequeue_input_buffer(0) {
            -1 => Ok(false),
            index => {
                let index = index as c_ulong;
                let buffer = self.get_input_buffer(index)?;

                let min_len = data.len().min(buffer.len());
                buffer[..min_len].copy_from_slice(&data[..min_len]);

                let flags = if end_of_stream {
                    ndk_sys::AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM as u32
                } else {
                    0
                };

                self.queue_input_buffer(index, 0, min_len as c_ulong, time, flags)?;
                Ok(true)
            }
        }
    }

    pub(crate) fn try_render(&self) -> anyhow::Result<bool> {
        const TRY_AGAIN_LATER: c_long = ndk_sys::AMEDIACODEC_INFO_TRY_AGAIN_LATER as c_long;
        const OUTPUT_FORMAT_CHANGED: c_long =
            ndk_sys::AMEDIACODEC_INFO_OUTPUT_FORMAT_CHANGED as c_long;
        const OUTPUT_BUFFERS_CHANGED: c_long =
            ndk_sys::AMEDIACODEC_INFO_OUTPUT_BUFFERS_CHANGED as c_long;

        let mut buffer_info = AMediaCodecBufferInfo {
            offset: 0,
            size: 0,
            presentationTimeUs: 0,
            flags: 0,
        };
        match self.dequeue_output_buffer(&mut buffer_info, 0) {
            TRY_AGAIN_LATER => Ok(false),
            // ignoring format change assuming the underlying surface can handle it
            OUTPUT_FORMAT_CHANGED => Ok(false),
            // deprecated in API level 21 and this is using 23 as minimum
            OUTPUT_BUFFERS_CHANGED => Ok(false),
            index => {
                self.release_output_buffer(index as c_ulong, true)?;
                Ok(true)
            }
        }
    }

    fn configure(
        &mut self,
        format: &MediaFormat,
        window: &NativeWindow,
    ) -> Result<(), MediaStatus> {
        unsafe {
            AMediaCodec_configure(
                self.as_inner(),
                format.as_inner(),
                window.as_inner(),
                std::ptr::null_mut(),
                0,
            )
            .success()
        }
    }

    fn start(&self) -> Result<(), MediaStatus> {
        unsafe { AMediaCodec_start(self.as_inner()).success() }
    }

    fn stop(&self) -> Result<(), MediaStatus> {
        unsafe { AMediaCodec_stop(self.as_inner()).success() }
    }

    fn dequeue_input_buffer(&self, timeout_us: i64) -> c_long {
        unsafe { AMediaCodec_dequeueInputBuffer(self.as_inner(), timeout_us) }
    }

    fn get_input_buffer(&self, index: c_ulong) -> anyhow::Result<&mut [u8]> {
        let mut buf_size = 0;
        unsafe {
            let buf_ptr = AMediaCodec_getInputBuffer(self.as_inner(), index, &mut buf_size);
            if buf_ptr.is_null() {
                anyhow::bail!("`AMediaCodec_getInputBuffer` returned a null");
            }
            Ok(std::slice::from_raw_parts_mut(buf_ptr, buf_size as usize))
        }
    }

    fn queue_input_buffer(
        &self,
        index: c_ulong,
        offset: i64,
        size: c_ulong,
        time: u64,
        flags: u32,
    ) -> Result<(), MediaStatus> {
        unsafe {
            AMediaCodec_queueInputBuffer(self.as_inner(), index, offset, size, time, flags)
                .success()
        }
    }

    fn dequeue_output_buffer(
        &self,
        buffer_info: &mut AMediaCodecBufferInfo,
        timeout_us: i64,
    ) -> c_long {
        unsafe { AMediaCodec_dequeueOutputBuffer(self.as_inner(), buffer_info, timeout_us) }
    }

    fn release_output_buffer(&self, index: c_ulong, render: bool) -> Result<(), MediaStatus> {
        unsafe { AMediaCodec_releaseOutputBuffer(self.as_inner(), index, render).success() }
    }
}

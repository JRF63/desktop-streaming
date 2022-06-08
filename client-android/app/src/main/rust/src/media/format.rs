use ndk_sys::{
    AMediaFormat, AMediaFormat_delete, AMediaFormat_getString, AMediaFormat_new,
    AMediaFormat_setBuffer, AMediaFormat_setInt32, AMediaFormat_setString,
    AMEDIAFORMAT_KEY_BIT_RATE, AMEDIAFORMAT_KEY_FRAME_RATE, AMEDIAFORMAT_KEY_HEIGHT,
    AMEDIAFORMAT_KEY_MAX_HEIGHT, AMEDIAFORMAT_KEY_MAX_WIDTH, AMEDIAFORMAT_KEY_MIME,
    AMEDIAFORMAT_KEY_PRIORITY, AMEDIAFORMAT_KEY_WIDTH,
};
use std::{os::raw::c_char, ptr::NonNull};

// `AMEDIAFORMAT_KEY_CSD_0` and `AMEDIAFORMAT_KEY_CSD_1` only became available in API level 28
const MEDIAFORMAT_KEY_CSD_0: &'static str = "csd-0\0";
const MEDIAFORMAT_KEY_CSD_1: &'static str = "csd-1\0";

#[repr(transparent)]
pub(crate) struct MediaFormat(NonNull<AMediaFormat>);

impl Drop for MediaFormat {
    fn drop(&mut self) {
        unsafe {
            AMediaFormat_delete(self.0.as_ptr());
        }
    }
}

impl MediaFormat {
    pub(crate) fn create_video_format(
        video_type: VideoType,
        width: i32,
        height: i32,
        frame_rate: i32,
        csd: &[u8],
    ) -> anyhow::Result<Self> {
        let mut media_format = {
            let ptr = unsafe { AMediaFormat_new() };
            match NonNull::new(ptr) {
                Some(media_format) => MediaFormat(media_format),
                None => anyhow::bail!("AMediaFormat_new returned a null"),
            }
        };

        media_format.set_video_type(video_type);
        media_format.set_width(width);
        media_format.set_height(height);
        media_format.set_frame_rate(frame_rate);

        // Used for adaptive playback
        media_format.set_max_width(width);
        media_format.set_max_height(height);

        match video_type {
            VideoType::H264 => match H264Csd::from_slice(csd) {
                Some(h264_csd) => h264_csd.add_to_format(&mut media_format),
                None => anyhow::bail!("Invalid codec specific data"),
            },
            VideoType::Hevc => match HevcCsd::from_slice(csd) {
                Some(hevc_csd) => hevc_csd.add_to_format(&mut media_format),
                None => anyhow::bail!("Invalid codec specific data"),
            },
        }
        Ok(media_format)
    }

    pub(crate) fn as_inner(&self) -> *mut AMediaFormat {
        self.0.as_ptr()
    }

    pub(crate) fn get_mime_type(&self) -> *const c_char {
        unsafe {
            // Resulting string is owned by the `AMediaFormat`
            let mut cstr: *const c_char = std::ptr::null();
            AMediaFormat_getString(self.as_inner(), AMEDIAFORMAT_KEY_MIME, &mut cstr);
            cstr
        }
    }

    fn set_video_type(&mut self, video_type: VideoType) {
        unsafe {
            AMediaFormat_setString(
                self.as_inner(),
                AMEDIAFORMAT_KEY_MIME,
                video_type.as_cstr_ptr(),
            );
        }
    }

    fn set_width(&mut self, width: i32) {
        unsafe {
            AMediaFormat_setInt32(self.as_inner(), AMEDIAFORMAT_KEY_WIDTH, width);
        }
    }

    fn set_max_width(&mut self, width: i32) {
        unsafe {
            AMediaFormat_setInt32(self.as_inner(), AMEDIAFORMAT_KEY_MAX_WIDTH, width);
        }
    }

    fn set_height(&mut self, height: i32) {
        unsafe {
            AMediaFormat_setInt32(self.as_inner(), AMEDIAFORMAT_KEY_HEIGHT, height);
        }
    }

    fn set_max_height(&mut self, height: i32) {
        unsafe {
            AMediaFormat_setInt32(self.as_inner(), AMEDIAFORMAT_KEY_MAX_HEIGHT, height);
        }
    }

    fn set_frame_rate(&mut self, frame_rate: i32) {
        unsafe {
            AMediaFormat_setInt32(self.as_inner(), AMEDIAFORMAT_KEY_FRAME_RATE, frame_rate);
        }
    }

    fn set_bit_rate(&mut self, bit_rate: i32) {
        unsafe {
            AMediaFormat_setInt32(self.as_inner(), AMEDIAFORMAT_KEY_BIT_RATE, bit_rate);
        }
    }

    fn set_realtime_priority(&mut self, realtime: bool) {
        unsafe {
            AMediaFormat_setInt32(
                self.as_inner(),
                AMEDIAFORMAT_KEY_PRIORITY,
                if realtime { 0 } else { 1 },
            );
        }
    }

    fn set_buffer(&mut self, name: *const c_char, data: &[u8]) {
        unsafe {
            AMediaFormat_setBuffer(
                self.as_inner(),
                name,
                data.as_ptr().cast(),
                data.len() as u64,
            )
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum VideoType {
    H264,
    Hevc,
}

impl VideoType {
    pub(crate) fn as_cstr_ptr(&self) -> *const std::os::raw::c_char {
        self.mime_cstr().as_ptr().cast()
    }

    fn mime_cstr(&self) -> &'static str {
        match self {
            VideoType::H264 => "video/avc\0",
            VideoType::Hevc => "video/hevc\0",
        }
    }
}

/// Find the starting positions of the [0x0, 0x0, 0x0, 0x1] marker.
fn nal_boundaries(data: &[u8]) -> Vec<usize> {
    let mut boundaries = Vec::with_capacity(3);

    let mut zeroes = 0;
    for (i, &byte) in data.iter().enumerate() {
        match byte {
            0 => zeroes += 1,
            1 => {
                if zeroes == 3 {
                    boundaries.push(i - 3);
                }
                zeroes = 0;
            }
            _ => zeroes = 0,
        }
    }
    boundaries
}

/// Used for manually setting H264 specific data. `AMediaFormat_setBuffer` with
/// `AMEDIAFORMAT_KEY_CSD_AVC` (API level >=29) can be used to pass the CSD buffer as a whole.
struct H264Csd<'a> {
    csd0: &'a [u8],
    csd1: &'a [u8],
}

impl<'a> H264Csd<'a> {
    /// Create a `H264Csd` from a byte buffer. This involves finding where the SPS and PPS are in
    /// the buffer. Returns `None` if they cannot be found.
    fn from_slice(data: &'a [u8]) -> Option<Self> {
        const SPS_NAL_UNIT_TYPE: u8 = 7;
        const PPS_NAL_UNIT_TYPE: u8 = 8;
        const NAL_UNIT_TYPE_MASK: u8 = 0b11111;

        let mut csd0 = None;
        let mut csd1 = None;

        let mut check_nal_type = |data: &'a [u8]| -> Option<()> {
            match data.get(4)? & NAL_UNIT_TYPE_MASK {
                SPS_NAL_UNIT_TYPE => csd0 = Some(data),
                PPS_NAL_UNIT_TYPE => csd1 = Some(data),
                _ => (),
            }
            Some(())
        };

        let boundaries = nal_boundaries(data);

        if boundaries.len() != 2 {
            return None;
        }

        let first = data.get(boundaries[0]..boundaries[1])?;
        let second = data.get(boundaries[1]..)?;

        check_nal_type(first)?;
        check_nal_type(second)?;

        Some(H264Csd {
            csd0: csd0?,
            csd1: csd1?,
        })
    }

    /// Include the content specific data in the format.
    fn add_to_format(&self, media_format: &mut MediaFormat) {
        media_format.set_buffer(MEDIAFORMAT_KEY_CSD_0.as_ptr().cast(), self.csd0);
        media_format.set_buffer(MEDIAFORMAT_KEY_CSD_1.as_ptr().cast(), self.csd1);
    }
}

/// Used for manually setting HEVC specific data. `AMediaFormat_setBuffer` with
/// `AMEDIAFORMAT_KEY_CSD_HEVC` (API level >=29) can be used instead.
struct HevcCsd<'a> {
    csd0: &'a [u8],
}

impl<'a> HevcCsd<'a> {
    /// Create a `HevcCsd` from a byte buffer. This needs to check for the presence of VPS, SPS and
    /// PPS NALs. Returns `None` if it fails.
    fn from_slice(_data: &'a [u8]) -> Option<Self> {
        todo!()
    }

    /// Include the content specific data in the format.
    fn add_to_format(&self, media_format: &mut MediaFormat) {
        media_format.set_buffer(MEDIAFORMAT_KEY_CSD_0.as_ptr().cast(), self.csd0);
    }
}

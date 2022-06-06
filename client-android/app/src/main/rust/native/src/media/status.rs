use std::num::NonZeroI32;

#[derive(Debug)]
pub(crate) struct MediaStatus(NonZeroI32);

impl MediaStatus {
    fn err_str(&self) -> &'static str {
        match self.0.get() {
            ndk_sys::media_status_t_AMEDIA_OK => "AMEDIA_OK",
            ndk_sys::media_status_t_AMEDIACODEC_ERROR_INSUFFICIENT_RESOURCE => {
                "AMEDIACODEC_ERROR_INSUFFICIENT_RESOURCE"
            }
            ndk_sys::media_status_t_AMEDIACODEC_ERROR_RECLAIMED => "AMEDIACODEC_ERROR_RECLAIMED",
            // AMEDIA_DRM_ERROR_BASE is the same as AMEDIA_ERROR_UNKNOWN
            ndk_sys::media_status_t_AMEDIA_ERROR_UNKNOWN => "AMEDIA_ERROR_UNKNOWN",
            ndk_sys::media_status_t_AMEDIA_ERROR_MALFORMED => "AMEDIA_ERROR_MALFORMED",
            ndk_sys::media_status_t_AMEDIA_ERROR_UNSUPPORTED => "AMEDIA_ERROR_UNSUPPORTED",
            ndk_sys::media_status_t_AMEDIA_ERROR_INVALID_OBJECT => "AMEDIA_ERROR_INVALID_OBJECT",
            ndk_sys::media_status_t_AMEDIA_ERROR_INVALID_PARAMETER => {
                "AMEDIA_ERROR_INVALID_PARAMETER"
            }
            ndk_sys::media_status_t_AMEDIA_ERROR_INVALID_OPERATION => {
                "AMEDIA_ERROR_INVALID_OPERATION"
            }
            ndk_sys::media_status_t_AMEDIA_ERROR_END_OF_STREAM => "AMEDIA_ERROR_END_OF_STREAM",
            ndk_sys::media_status_t_AMEDIA_ERROR_IO => "AMEDIA_ERROR_IO",
            ndk_sys::media_status_t_AMEDIA_ERROR_WOULD_BLOCK => "AMEDIA_ERROR_WOULD_BLOCK",
            ndk_sys::media_status_t_AMEDIA_DRM_ERROR_BASE => "AMEDIA_DRM_ERROR_BASE",
            ndk_sys::media_status_t_AMEDIA_DRM_NOT_PROVISIONED => "AMEDIA_DRM_NOT_PROVISIONED",
            ndk_sys::media_status_t_AMEDIA_DRM_RESOURCE_BUSY => "AMEDIA_DRM_RESOURCE_BUSY",
            ndk_sys::media_status_t_AMEDIA_DRM_DEVICE_REVOKED => "AMEDIA_DRM_DEVICE_REVOKED",
            ndk_sys::media_status_t_AMEDIA_DRM_SHORT_BUFFER => "AMEDIA_DRM_SHORT_BUFFER",
            ndk_sys::media_status_t_AMEDIA_DRM_SESSION_NOT_OPENED => {
                "AMEDIA_DRM_SESSION_NOT_OPENED"
            }
            ndk_sys::media_status_t_AMEDIA_DRM_TAMPER_DETECTED => "AMEDIA_DRM_TAMPER_DETECTED",
            ndk_sys::media_status_t_AMEDIA_DRM_VERIFY_FAILED => "AMEDIA_DRM_VERIFY_FAILED",
            ndk_sys::media_status_t_AMEDIA_DRM_NEED_KEY => "AMEDIA_DRM_NEED_KEY",
            ndk_sys::media_status_t_AMEDIA_DRM_LICENSE_EXPIRED => "AMEDIA_DRM_LICENSE_EXPIRED",
            ndk_sys::media_status_t_AMEDIA_IMGREADER_ERROR_BASE => "AMEDIA_IMGREADER_ERROR_BASE",
            ndk_sys::media_status_t_AMEDIA_IMGREADER_NO_BUFFER_AVAILABLE => {
                "AMEDIA_IMGREADER_NO_BUFFER_AVAILABLE"
            }
            ndk_sys::media_status_t_AMEDIA_IMGREADER_MAX_IMAGES_ACQUIRED => {
                "AMEDIA_IMGREADER_MAX_IMAGES_ACQUIRED"
            }
            ndk_sys::media_status_t_AMEDIA_IMGREADER_CANNOT_LOCK_IMAGE => {
                "AMEDIA_IMGREADER_CANNOT_LOCK_IMAGE"
            }
            ndk_sys::media_status_t_AMEDIA_IMGREADER_CANNOT_UNLOCK_IMAGE => {
                "AMEDIA_IMGREADER_CANNOT_UNLOCK_IMAGE"
            }
            ndk_sys::media_status_t_AMEDIA_IMGREADER_IMAGE_NOT_LOCKED => {
                "AMEDIA_IMGREADER_IMAGE_NOT_LOCKED"
            }
            _ => "MediaStatus unknown error",
        }
    }
}

impl std::fmt::Display for MediaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.err_str())
    }
}

impl std::error::Error for MediaStatus {}

pub(crate) trait AsMediaStatus {
    fn success(self) -> Result<(), MediaStatus>;
}

impl AsMediaStatus for i32 {
    fn success(self) -> Result<(), MediaStatus> {
        match NonZeroI32::new(self) {
            Some(nonzero) => Err(MediaStatus(nonzero)),
            None => Ok(()), // AMEDIA_OK
        }
    }
}

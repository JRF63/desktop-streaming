mod error_strings;

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum NvEncError {
    // NOTE: This is probably not as efficient. Listing the `NVENCSTATUS` variants (excluding
    // NV_ENC_SUCCESS) directly into this enum allows `Option<NvEncError>` to encode `None` as a
    // zero which *may* be faster to check.
    // Consider creating a `NvEncStatusExceptSuccess` enum.
    #[error("{}", error_strings::nvenc_status_to_str(.0))]
    Sys(crate::sys::NVENCSTATUS),

    // TODO: Maybe split these into separate enums

    #[error("The shared library for `nvEncodeAPI64` is not signed and may have been tampered.")]
    LibraryNotSigned,
    #[error("Loading the shared library for `nvEncodeAPI64` failed.")]
    LibraryLoadingFailed,
    #[error("Unable to locate `NvEncodeAPIGetMaxSupportedVersion` in the shared library.")]
    GetMaxSupportedVersionLoadingFailed,
    #[error("Unable to locate `NvEncodeAPICreateInstance` in the shared library.")]
    CreateInstanceLoadingFailed,
    #[error("The installed driver does not support the version of the NvEnc API that this library is compiled with.")]
    UnsupportedVersion,
    #[error("`NvEncodeAPICreateInstance` returned a malformed function list.")]
    MalformedFunctionList,

    #[error("The encoder for the current device does not support the codec")]
    UnsupportedCodec,
    #[error("Codec needs to be set first")]
    CodecNotSet,
    #[error("The encoder does not support the given codec profile for the current codec")]
    CodecProfileNotSupported,
    #[error("Encode preset is needed to build the encoder")]
    EncodePresetNotSet,

    #[error("Failed creating a texture buffer")]
    TextureBufferCreationFailed,

    #[error("Could not create a Windows event object")]
    EventObjectCreationFailed,
    #[error("Error while waiting for the event object to be signaled")]
    EventObjectWaitError,
    #[error("Event timed-out while waiting")]
    EventObjectWaitTimeout,

    #[error("Input has signaled end of stream")]
    EndOfStream,
}

impl Default for NvEncError {
    #[inline]
    fn default() -> Self {
        NvEncError::Sys(crate::sys::NVENCSTATUS::NV_ENC_ERR_GENERIC)
    }
}

impl NvEncError {
    /// Create a `NvEncError` from a `NVENCSTATUS`. Returns `None` if `status` is
    /// NVENCSTATUS::NV_ENC_SUCCESS.
    #[inline]
    pub fn from_nvenc_status(status: crate::sys::NVENCSTATUS) -> Option<Self> {
        match status {
            crate::sys::NVENCSTATUS::NV_ENC_SUCCESS => None,
            status => Some(NvEncError::Sys(status)),
        }
    }

    /// Try to convert `NvEncError` into a `NVENCSTATUS`.
    #[inline]
    pub fn into_nvenc_status(self) -> Option<crate::sys::NVENCSTATUS> {
        match self {
            NvEncError::Sys(status) => Some(status),
            _ => None,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nv_enc_success_is_zero() {
        let status = crate::sys::NVENCSTATUS::NV_ENC_SUCCESS;
        assert_eq!(status as i32, 0);
    }

    #[test]
    fn error_same_size() {
        assert_eq!(
            std::mem::size_of::<crate::sys::NVENCSTATUS>(),
            std::mem::size_of::<NvEncError>()
        );
    }

    #[test]
    fn option_error_same_size() {
        assert_eq!(
            std::mem::size_of::<Option<NvEncError>>(),
            std::mem::size_of::<NvEncError>()
        );
    }
}

use crate::sys;

#[derive(thiserror::Error, Debug)]
#[repr(i32)]
pub enum Error {
    #[error("One or more invalid/out of range arguments")]
    BadArg = sys::OPUS_BAD_ARG,
    #[error("Not enough bytes allocated in the buffer")]
    BufferTooSmall = sys::OPUS_BUFFER_TOO_SMALL,
    #[error("An internal error was detected")]
    InternalError = sys::OPUS_INTERNAL_ERROR,
    #[error("The compressed data passed is corrupted")]
    InvalidPacket = sys::OPUS_INVALID_PACKET,
    #[error("Invalid/unsupported request number")]
    Unimplemented = sys::OPUS_UNIMPLEMENTED,
    #[error("An encoder or decoder structure is invalid or already freed")]
    InvalidState = sys::OPUS_INVALID_STATE,
    #[error("Memory allocation has failed")]
    AllocFail = sys::OPUS_ALLOC_FAIL,
    #[error("The provided bitrate falls outside the supported range")]
    InvalidBitrate = sys::OPUS_ALLOC_FAIL - 1,
}

impl Error {
    /// Converts an error code to an `Error`. Returns `None` if the error code is not an error
    /// (`0`).
    ///
    /// This function _can_ only accept error codes in the range `(-7)..0` and passing an error
    /// code outside the range is undefined behavior.
    pub unsafe fn from_raw_error_code(error_code: i32) -> Option<Error> {
        match error_code {
            sys::OPUS_OK => None,
            sys::OPUS_BAD_ARG => Some(Error::BadArg),
            sys::OPUS_BUFFER_TOO_SMALL => Some(Error::BufferTooSmall),
            sys::OPUS_INTERNAL_ERROR => Some(Error::InternalError),
            sys::OPUS_INVALID_PACKET => Some(Error::InvalidPacket),
            sys::OPUS_UNIMPLEMENTED => Some(Error::Unimplemented),
            sys::OPUS_INVALID_STATE => Some(Error::InvalidState),
            sys::OPUS_ALLOC_FAIL => Some(Error::AllocFail),
            _ => std::hint::unreachable_unchecked(),
        }
    }
}

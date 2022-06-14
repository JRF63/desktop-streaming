mod encoder;
mod error;
mod settings;
mod sync;
mod util;

pub(crate) mod os;

#[macro_export]
macro_rules! nvenc_function {
    ($fn:expr, $($arg:expr),*) => {
        let status = ($fn.unwrap_or_else(|| std::hint::unreachable_unchecked()))($($arg,)*);
        if let Some(error) = crate::error::NvEncError::from_nvenc_status(status) {
            return Err(error.into());
        }
    }
}

pub type Result<T> = std::result::Result<T, NvEncError>;

pub use encoder::create_encoder;
pub use error::NvEncError;
pub use settings::{Codec, CodecProfile, EncoderPreset, TuningInfo};

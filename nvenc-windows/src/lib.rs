mod encoder;
mod error;
mod guids;
mod util;

#[macro_export]
macro_rules! nvenc_function {
    ($fn:expr, $($arg:expr),*) => {
        let status = ($fn.unwrap_or_else(|| std::hint::unreachable_unchecked()))($($arg,)*);
        if status != nvenc_sys::NVENCSTATUS::NV_ENC_SUCCESS {
            return Err(crate::error::NvEncError::new(status));
        }
    }
}

pub use encoder::create_encoder;
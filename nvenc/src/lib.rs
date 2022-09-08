mod encoder;
mod error;
mod settings;
mod util;

pub(crate) mod os;

pub type Result<T> = std::result::Result<T, NvEncError>;

pub use encoder::create_encoder;
pub use error::NvEncError;
pub use settings::{Codec, CodecProfile, EncoderPreset, TuningInfo};

mod encoder;
mod error;
mod settings;
mod sys;
mod util;

pub mod os;

pub type Result<T> = std::result::Result<T, NvEncError>;

pub use encoder::create_encoder;
pub use error::NvEncError;
pub use settings::{Codec, CodecProfile, EncoderPreset, TuningInfo};

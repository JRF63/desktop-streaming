mod encoder;
mod error;
mod settings;
mod sys;
mod util;

pub type Result<T> = std::result::Result<T, NvEncError>;

pub use self::{
    encoder::{Device, EncoderBuilder, EncoderInput, EncoderOutput, IntoDevice},
    error::NvEncError,
    settings::{Codec, CodecProfile, EncodePreset, TuningInfo},
};

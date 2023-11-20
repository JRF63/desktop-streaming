mod windows;

pub use crate::windows::*;

#[repr(u16)]
pub enum AudioFormatType {
    Pcm,
    IeeeFloat,
}

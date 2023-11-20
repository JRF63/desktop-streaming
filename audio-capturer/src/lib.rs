mod windows;

pub use crate::windows::*;

const NUM_CHANNELS: u32 = 2;
const OPUS_BITS_PER_SAMPLE: u32 = 16;

#[repr(u16)]
pub enum AudioFormatType {
    Pcm,
    IeeeFloat,
}

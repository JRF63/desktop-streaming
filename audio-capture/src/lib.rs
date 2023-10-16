mod windows;

pub use crate::windows::*;

const NUM_CHANNELS: u32 = 2;
const OPUS_BITRATE: u32 = 48000;
const OPUS_BITS_PER_SAMPLE: u32 = 16;
use std::{marker::PhantomData, ptr::NonNull};

#[derive(Debug)]
pub struct AudioData<'a> {
    pub data: NonNull<u8>,
    pub num_frames: u32,
    pub flags: u32,
    pub timestamp: u64,
    phantom: PhantomData<&'a [u8]>,
}

impl<'a> AudioData<'a> {
    pub fn new(data: NonNull<u8>, num_frames: u32, flags: u32, timestamp: u64) -> Self {
        Self {
            data,
            num_frames,
            flags,
            timestamp,
            phantom: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum AudioFormatType {
    Pcm,
    IeeeFloat,
}

pub(crate) trait AudioDataDrop {
    fn drop_audio_data<'a>(&self, audio_data: &'a AudioData<'a>);
}
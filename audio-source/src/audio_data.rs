use std::{marker::PhantomData, ptr::NonNull};

#[derive(Debug)]
pub struct AudioData<'a> {
    pub data: NonNull<u8>, // Must be aligned to both `i16` and `f32`
    pub num_frames: u32,
    pub flags: u32,
    pub timestamp: u64,
    phantom: PhantomData<&'a [u8]>,
}

impl<'a> AudioData<'a> {
    pub(crate) fn new(data: NonNull<u8>, num_frames: u32, flags: u32, timestamp: u64) -> Self {
        Self {
            data,
            num_frames,
            flags,
            timestamp,
            phantom: PhantomData,
        }
    }
}

/// The kind of output of the audio source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum AudioFormatKind {
    /// 16-bit PCM
    Pcm,
    /// 32-bit IEEE floating-point
    IeeeFloat,
}

pub trait AudioDataDrop {
    fn drop_audio_data<'a>(&self, audio_data: &'a AudioData<'a>);
}

/// RAII wrapper for automatically freeing the buffer returned by `get_audio_data`.
pub struct AudioDataWrapper<'a, T>
where
    T: AudioDataDrop,
{
    inner: AudioData<'a>,
    parent: &'a T,
}

impl<'a, T> Drop for AudioDataWrapper<'a, T>
where
    T: AudioDataDrop,
{
    fn drop(&mut self) {
        self.parent.drop_audio_data(&self.inner);
    }
}

impl<'a, T> std::ops::Deref for AudioDataWrapper<'a, T>
where
    T: AudioDataDrop,
{
    type Target = AudioData<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T> AudioDataWrapper<'a, T>
where
    T: AudioDataDrop,
{
    pub(crate) fn new(inner: AudioData<'a>, parent: &'a T) -> Self {
        Self { inner, parent }
    }
}

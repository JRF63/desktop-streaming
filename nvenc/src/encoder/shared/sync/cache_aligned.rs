// Modified from https://doc.rust-lang.org/src/std/sync/mpsc/cache_aligned.rs.html

use std::ops::{Deref, DerefMut};

#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(align(128))]
pub struct CacheAligned<T>(T);

impl<T> Deref for CacheAligned<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for CacheAligned<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> CacheAligned<T> {
    #[inline]
    pub(super) fn new(inner: T) -> Self {
        CacheAligned(inner)
    }
}

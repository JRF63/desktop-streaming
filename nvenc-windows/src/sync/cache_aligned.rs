// Modified from https://doc.rust-lang.org/src/std/sync/mpsc/cache_aligned.rs.html

use std::ops::{Deref, DerefMut};

#[repr(align(128))]
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct CacheAligned<T>(T);

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

    #[inline]
    pub(super) fn into_inner(self) -> T {
        self.0
    }
}

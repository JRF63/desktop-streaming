use super::cache_aligned::CacheAligned;
use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::AtomicUsize,
};

// Implementation was taken from:
// https://www.snellman.net/blog/archive/2016-12-13-ring-buffers/

/// A collection that allows reads and writes to the same buffer from different threads by
/// managing the indices of the receiver and the sender.
/// The underlying buffer has valid, initialized data but this behaves like a queue in that
/// something must be written before it can be read and the item cannot be read again until after
/// the next write.
#[repr(C)]
pub(super) struct CyclicBuffer<T, const N: usize> {
    /// Index of the sender, read by the receiver buy only written to by the sender
    pub(super) head: AtomicUsize,
    /// Index of the receiver, read by the sender buy only written to by the receiver
    pub(super) tail: AtomicUsize,
    /// Array that holds the items
    pub(super) buffer: [UnsafeCell<CacheAligned<T>>; N],
}

impl<T, const N: usize> CyclicBuffer<T, N> {
    /// Creates a new `CyclicBuffer`. Returns `None` if the buffer size is not a power of two.
    pub(super) fn new(buffer: [T; N]) -> Option<Self> {
        if N & (N - 1) != 0 {
            return None;
        }

        let mut tmp = MaybeUninit::<[UnsafeCell<CacheAligned<T>>; N]>::uninit();
        unsafe {
            // Pointer to the start of the array's buffer
            let mut ptr = (&mut *tmp.as_mut_ptr()).as_mut_ptr();
            for item in buffer {
                ptr.write(UnsafeCell::new(CacheAligned::new(item)));
                ptr = ptr.offset(1);
            }
        }
        Some(CyclicBuffer {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            buffer: unsafe { tmp.assume_init() },
        })
    }
}
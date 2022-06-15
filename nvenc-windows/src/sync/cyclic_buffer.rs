use super::cache_aligned::CacheAligned;
use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A collection that allows reads and writes to the same buffer from different threads by
/// managing the indices of the reader and the writer.
/// The underlying buffer has valid, initialized data but this behaves like a queue in that
/// something must be written before it can be read and the item cannot be read again until after
/// the next write.
#[repr(C)]
pub(crate) struct CyclicBuffer<T, const N: usize> {
    /// Index of the writer
    head: AtomicUsize,
    /// Index of the reader
    tail: AtomicUsize,
    /// Array that holds the items
    buffer: [UnsafeCell<CacheAligned<T>>; N],
}

unsafe impl<T, const N: usize> Send for CyclicBuffer<T, N> where T: Send {}
unsafe impl<T, const N: usize> Sync for CyclicBuffer<T, N> where T: Send {}

impl<T, const N: usize> CyclicBuffer<T, N> {
    /// Creates a new `CyclicBuffer`.
    #[inline]
    pub(crate) fn new(buffer: [T; N]) -> Self {
        let mut tmp = MaybeUninit::<[UnsafeCell<CacheAligned<T>>; N]>::uninit();
        unsafe {
            // Pointer to the start of the array's buffer
            let mut ptr = (&mut *tmp.as_mut_ptr()).as_mut_ptr();
            for item in buffer {
                ptr.write(UnsafeCell::new(CacheAligned::new(item)));
                ptr = ptr.offset(1);
            }
        }
        CyclicBuffer {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            buffer: unsafe { tmp.assume_init() },
        }
    }

    /// Modify an item on the buffer. Blocks if the buffer is full.
    #[inline]
    pub(crate) fn writer_access<F>(&self, mut modify_op: F)
    where
        F: FnMut(usize, &mut T),
    {
        // `Ordering::Release` would be possible if `CyclicBuffer` is purposely not `Send` since
        // in that case, the value read here would only be from the last `Ordering::Release` on the
        // same thread.
        let head = self.head.load(Ordering::Acquire);
        loop {
            let tail = self.tail.load(Ordering::Acquire);

            // Break if not full
            if (head - tail) <= N {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = head & (N - 1);
        unsafe {
            let cell = self.buffer.get_unchecked(index);
            modify_op(index, &mut *cell.get());
        }

        self.head.store(head + 1, Ordering::Release);
    }

    /// Read an item on the buffer. Blocks if the buffer is empty.
    #[inline]
    pub(crate) fn reader_access<F, E>(&self, mut read_op: F) -> Result<(), E>
    where
        F: FnMut(&T) -> Result<(), E>,
    {
        // Same reasoning as on `modify`
        let tail = self.tail.load(Ordering::Acquire);
        loop {
            let head = self.head.load(Ordering::Acquire);

            // Break if not empty
            if head != tail {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = tail & (N - 1);
        unsafe {
            let cell = self.buffer.get_unchecked(index);
            read_op(&*cell.get())?;
        }

        self.tail.store(tail + 1, Ordering::Release);
        Ok(())
    }

    /// Returns the internal buffer. `&mut self` guarantees exclusive access from a single thread.
    pub(crate) fn get_mut(&mut self) -> &mut [UnsafeCell<CacheAligned<T>>; N] {
        &mut self.buffer
    }
}

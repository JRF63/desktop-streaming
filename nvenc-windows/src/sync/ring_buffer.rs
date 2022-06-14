use super::cache_aligned::CacheAligned;
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// A ring buffer intended to be used by a single consumer, single producer
/// channel.
#[repr(C)]
pub(super) struct WindowBuffer<T, const N: usize> {
    /// The index that is moved by a `push` operation
    head: AtomicUsize,
    /// Index where the item to be popped is located
    tail: AtomicUsize,
    /// Array that holds the items
    buffer: [UnsafeCell<CacheAligned<T>>; N],
    /// Used to signal if `Receiver` is to be stopped
    stopped: AtomicBool,
}

unsafe impl<T: Send, const N: usize> Send for WindowBuffer<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for WindowBuffer<T, N> {}

impl<T, const N: usize> WindowBuffer<T, N> {
    /// Creates a new `WindowBuffer`.
    #[inline]
    pub(super) fn new() -> Self {
        WindowBuffer {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            buffer: unsafe { std::mem::MaybeUninit::uninit().assume_init() },
            stopped: AtomicBool::new(false),
        }
    }

    /// Prevents further popping from the buffer. To be used by `Sender`.
    #[inline]
    pub(super) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    /// Checks if buffer is stopped. To be used by `Receiver`.
    #[inline]
    pub(super) fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    /// Pushes a new item to the buffer. Blocks if buffer is full.
    #[inline]
    pub(super) fn push(&self, item: T) {
        let mut head = self.head.load(Ordering::Relaxed);
        loop {
            let tail = self.tail.load(Ordering::Acquire);

            // break if not full
            if (head - tail) < N {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = head & (N - 1);
        let cell = unsafe { self.buffer.get_unchecked(index) };
        unsafe {
            *cell.get() = CacheAligned::new(item);
        }

        head += 1;
        self.head.store(head, Ordering::Release);
    }

    /// Attempts to pop an item in the buffer. Returns `None` if buffer is empty.
    #[inline]
    pub(super) fn pop(&self) -> Option<T> {
        let mut tail = self.tail.load(Ordering::Relaxed);
        loop {
            let head = self.head.load(Ordering::Acquire);

            // break if not empty
            if head != tail {
                break;
            } else {
                return None;
            }
        }

        let index = tail & (N - 1);
        let cell = unsafe { self.buffer.get_unchecked(index) };
        let result = unsafe { cell.get().read() };

        tail += 1;
        self.tail.store(tail, Ordering::Release);

        Some(result.into_inner())
    }
}

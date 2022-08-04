use super::cyclic_buffer::CyclicBuffer;
use std::sync::{atomic::Ordering, Arc};

pub struct Reader<T, const N: usize> {
    inner: Arc<CyclicBuffer<T, N>>,
}

unsafe impl<T, const N: usize> Send for Reader<T, N> {}

impl<T, const N: usize> Reader<T, N> {
    pub(super) fn new(buffer: Arc<CyclicBuffer<T, N>>) -> Self {
        Reader { inner: buffer }
    }

    /// Read an item on the buffer. Blocks if the buffer is empty.
    pub fn read<F, R>(&self, mut read_op: F) -> R
    where
        F: FnMut(&T) -> R,
    {
        // Needs to synchronize-with the `store` below since this might be moved to another thread
        let tail = self.inner.tail.load(Ordering::Acquire);
        loop {
            let head = self.inner.head.load(Ordering::Acquire);

            // Proceed if not empty; `head` is not always >= `tail` because of wrap-around
            if head != tail {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = tail & (N - 1);
        let result = unsafe {
            let cell = self.inner.buffer.get_unchecked(index);
            read_op(&*cell.get())
        };

        self.inner
            .tail
            .store(tail.wrapping_add(1), Ordering::Release);
        result
    }
}

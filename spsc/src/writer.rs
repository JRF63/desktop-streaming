use super::cyclic_buffer::CyclicBuffer;
use std::sync::{atomic::Ordering, Arc};

pub struct Writer<T, const N: usize> {
    inner: Arc<CyclicBuffer<T, N>>,
}

unsafe impl<T, const N: usize> Send for Writer<T, N> {}

impl<T, const N: usize> Writer<T, N> {
    pub(super) fn new(buffer: Arc<CyclicBuffer<T, N>>) -> Self {
        Writer { inner: buffer }
    }

    /// Modify an item on the buffer. Blocks if the buffer is full.
    pub fn write<F, S, R>(&self, args: S, mut write_op: F) -> R
    where
        F: FnMut(usize, &mut T, S) -> R,
    {
        // Needs to synchronize-with the `store` below since this might be moved to another thread
        let head = self.inner.head.load(Ordering::Acquire);
        loop {
            let tail = self.inner.tail.load(Ordering::Acquire);

            // Proceed if not full; The indices can wrap around so `!=` must be used here
            if (head - tail) != N {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = head & (N - 1);
        let result = unsafe {
            let cell = self.inner.buffer.get_unchecked(index);
            write_op(index, &mut *cell.get(), args)
        };

        self.inner
            .head
            .store(head.wrapping_add(1), Ordering::Release);
        result
    }
}

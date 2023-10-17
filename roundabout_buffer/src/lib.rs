use crossbeam_utils::CachePadded;
use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

// Implementation modified from:
// https://www.snellman.net/blog/archive/2016-12-13-ring-buffers/

/// A collection that allows reads and writes to the same buffer from different threads by
/// managing the indices of the reader and the writer.
/// The underlying buffer has valid, initialized data but this behaves like a queue in that
/// something must be written before it can be read and the item cannot be read again until after
/// the next write.
#[repr(C)]
pub struct RoundaboutBuffer<T, const N: usize> {
    /// Index of the writer
    head: AtomicUsize,
    /// Index of the reader
    tail: AtomicUsize,
    /// Array that holds the items
    buffer: [UnsafeCell<CachePadded<T>>; N],
}

impl<T, const N: usize> RoundaboutBuffer<T, N> {
    /// Create a new reader/writer pair. The data structure *may* be more efficient if `N` is a
    /// power of two.
    pub fn channel(buffer: [T; N]) -> (RoundaboutBufferWriter<T, N>, RoundaboutBufferReader<T, N>) {
        let roundabout = Arc::new(RoundaboutBuffer {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            buffer: buffer.map(|x| UnsafeCell::new(CachePadded::new(x))),
        });
        let writer = RoundaboutBufferWriter(roundabout.clone());
        let reader = RoundaboutBufferReader(roundabout);
        (writer, reader)
    }

    /// Returns the internal buffer. `&mut self` guarantees exclusive access from a single thread.
    pub fn get_mut(&mut self) -> &mut [UnsafeCell<CachePadded<T>>; N] {
        &mut self.buffer
    }

    /// Maps `index` to [0, N) for use as an index to the inner buffer.
    fn map_to_valid_index(index: usize) -> usize {
        // Compiler should be able to optimize to `index & (N - 1)` when N is a power of two
        index % N
    }
}

#[repr(transparent)]
pub struct RoundaboutBufferWriter<T, const N: usize>(Arc<RoundaboutBuffer<T, N>>);

unsafe impl<T, const N: usize> Send for RoundaboutBufferWriter<T, N> where T: Send {}

impl<T, const N: usize> RoundaboutBufferWriter<T, N> {
    /// Modify an item on the buffer. Blocks if the buffer is full.
    #[inline]
    pub fn write<F, R>(&mut self, write_op: F) -> R
    where
        F: FnOnce(usize, &mut T) -> R,
    {
        // Needs to synchronize-with the `store` below since this might be moved to another thread
        let head = self.0.head.load(Ordering::Acquire);
        loop {
            let tail = self.0.tail.load(Ordering::Acquire);

            // Proceed if not full; The indices can wrap around so `!=` must be used here
            if (head - tail) != N {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = RoundaboutBuffer::<T, N>::map_to_valid_index(head);
        let result = unsafe {
            let cell = self.0.buffer.get_unchecked(index);
            write_op(index, &mut *cell.get())
        };

        self.0.head.store(head.wrapping_add(1), Ordering::Release);
        result
    }
}

#[repr(transparent)]
pub struct RoundaboutBufferReader<T, const N: usize>(Arc<RoundaboutBuffer<T, N>>);

unsafe impl<T, const N: usize> Send for RoundaboutBufferReader<T, N> where T: Send {}

impl<T, const N: usize> RoundaboutBufferReader<T, N> {
    /// Read an item on the buffer. Blocks if the buffer is empty.
    #[inline]
    pub fn read<F, R>(&mut self, read_op: F) -> R
    where
        F: FnOnce(usize, &T) -> R,
    {
        // Needs to synchronize-with the `store` below since this might be moved to another thread
        let tail = self.0.tail.load(Ordering::Acquire);
        loop {
            let head = self.0.head.load(Ordering::Acquire);

            // Proceed if not empty; `head` is not always >= `tail` because of wrap-around
            if head != tail {
                break;
            } else {
                std::thread::yield_now();
            }
        }

        let index = RoundaboutBuffer::<T, N>::map_to_valid_index(tail);
        let result = unsafe {
            let cell = self.0.buffer.get_unchecked(index);
            read_op(index, &*cell.get())
        };

        self.0.tail.store(tail.wrapping_add(1), Ordering::Release);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{
        distributions::{Distribution, Uniform},
        rngs::StdRng,
        SeedableRng,
    };
    use std::time::Duration;

    #[test]
    fn buffer_sanity_check() {
        std::thread::scope(|s| {
            const ITERS: i32 = 1000;

            let array = [0; 32];
            let (mut writer, mut reader) = RoundaboutBuffer::channel(array);

            let mut rng = StdRng::seed_from_u64(0);
            let write_delay_between = Uniform::from(1..100);
            let read_delay_between = Uniform::from(1..100);

            let write_delays: Vec<_> = (0..ITERS)
                .map(|_| Duration::from_micros(write_delay_between.sample(&mut rng)))
                .collect();

            let read_delays: Vec<_> = (0..ITERS)
                .map(|_| Duration::from_micros(read_delay_between.sample(&mut rng)))
                .collect();

            s.spawn(move || {
                for (i, sleep_dur) in write_delays.into_iter().enumerate() {
                    writer.write(|_, val| {
                        *val = i;
                    });
                    std::thread::sleep(sleep_dur);
                }
            });

            s.spawn(move || {
                for (i, sleep_dur) in read_delays.into_iter().enumerate() {
                    reader.read(|_, val| {
                        assert_eq!(i, *val);
                    });
                    std::thread::sleep(sleep_dur);
                }
            });
        });
    }
}

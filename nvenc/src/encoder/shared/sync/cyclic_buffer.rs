use super::cache_aligned::CacheAligned;
use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

// Implementation modified from:
// https://www.snellman.net/blog/archive/2016-12-13-ring-buffers/

/// A collection that allows reads and writes to the same buffer from different threads by
/// managing the indices of the reader and the writer.
/// The underlying buffer has valid, initialized data but this behaves like a queue in that
/// something must be written before it can be read and the item cannot be read again until after
/// the next write.
#[repr(C)]
pub struct CyclicBuffer<T, const N: usize> {
    /// Index of the writer
    head: AtomicUsize,
    /// Index of the reader
    tail: AtomicUsize,
    /// Array that holds the items
    buffer: [UnsafeCell<CacheAligned<T>>; N],
}

impl<T, const N: usize> CyclicBuffer<T, N> {
    /// Creates a new `CyclicBuffer`. Returns `None` if the buffer size is not a power of two or is
    /// zero.
    pub fn new(buffer: [T; N]) -> Option<Self> {
        if !is_power_of_two(N) {
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

    /// Returns the internal buffer. `&mut self` guarantees exclusive access from a single thread.
    pub fn get_mut(&mut self) -> &mut [UnsafeCell<CacheAligned<T>>; N] {
        &mut self.buffer
    }
}

#[repr(transparent)]
pub struct CyclicBufferWriter<T, const N: usize>(CyclicBuffer<T, N>);

impl<T, const N: usize> CyclicBufferWriter<T, N> {
    /// Reinterpret a `&CyclicBuffer` as a `CyclicBufferWriter`.
    pub unsafe fn from_shared_buffer(shared_buffer: &CyclicBuffer<T, N>) -> &Self {
        std::mem::transmute(shared_buffer)
    }

    /// Modify an item on the buffer. Blocks if the buffer is full.
    #[inline]
    pub fn write<F, R>(&self, write_op: F) -> R
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

        let index = head & (N - 1);
        let result = unsafe {
            let cell = self.0.buffer.get_unchecked(index);
            write_op(index, &mut *cell.get())
        };

        self.0.head.store(head.wrapping_add(1), Ordering::Release);
        result
    }
}

#[repr(transparent)]
pub struct CyclicBufferReader<T, const N: usize>(CyclicBuffer<T, N>);

impl<T, const N: usize> CyclicBufferReader<T, N> {
    /// Reinterpret a `&CyclicBuffer` as a `CyclicBufferWriter`.
    pub unsafe fn from_shared_buffer(shared_buffer: &CyclicBuffer<T, N>) -> &Self {
        std::mem::transmute(shared_buffer)
    }

    /// Read an item on the buffer. Blocks if the buffer is empty.
    #[inline]
    pub fn read<F, R>(&self, read_op: F) -> R
    where
        F: FnOnce(&T) -> R,
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

        let index = tail & (N - 1);
        let result = unsafe {
            let cell = self.0.buffer.get_unchecked(index);
            read_op(&*cell.get())
        };

        self.0.tail.store(tail.wrapping_add(1), Ordering::Release);
        result
    }
}

/// Tests if `num` is a power of two.
const fn is_power_of_two(num: usize) -> bool {
    num == 0 || num.count_ones() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_of_two() {
        let powers_of_two = vec![0, 1, 2, 4, 8, 16, 32, 64, 128];
        for &num in &powers_of_two {
            assert!(is_power_of_two(num));
        }

        for num in 0..128 {
            if !powers_of_two.contains(&num) {
                assert!(!is_power_of_two(num));
            }
        }
    }

    #[test]
    fn buffer_sanity_check() {
        use std::sync::Arc;

        struct DummyBuffer<T, const N: usize>(Arc<CyclicBuffer<T, N>>);

        unsafe impl<T, const N: usize> Send for DummyBuffer<T, N> {}

        // Helper function to restrict the usage of a `CyclicBuffer` between two threads only
        // (writer and the reader)
        fn dummy_channel<T, const N: usize>(buffer: [T; N]) -> (DummyBuffer<T, N>, DummyBuffer<T, N>) {
            let shared_buffer = Arc::new(CyclicBuffer::new(buffer).unwrap());
            let writer = DummyBuffer(shared_buffer.clone());
            let reader = DummyBuffer(shared_buffer);
            (writer, reader)
        }

        std::thread::scope(|s| {
            const ITERS: i32 = 1000;

            let array = [0; 8];
            let (writer, reader) = dummy_channel(array);

            s.spawn(move || {
                let writer = writer;
                let writer = unsafe { CyclicBufferWriter::from_shared_buffer(&writer.0) };
                for i in 0..ITERS {
                    writer.write(|_i, val| {
                        *val = i;
                    });
                }
            });

            s.spawn(|| {
                let reader = reader;
                let reader = unsafe { CyclicBufferReader::from_shared_buffer(&reader.0) };
                for i in 0..ITERS {
                    reader.read(|val| {
                        assert_eq!(i, *val);
                    });
                }
            });
        });
    }
}

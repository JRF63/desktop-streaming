use crossbeam_utils::CachePadded;
use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
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
pub struct ConveyorBuffer<T, const N: usize> {
    /// Index of the writer
    head: AtomicUsize,
    /// Index of the reader
    tail: AtomicUsize,
    /// Array that holds the items
    buffer: [UnsafeCell<CachePadded<T>>; N],
}

impl<T, const N: usize> ConveyorBuffer<T, N> {
    /// Retrieves the internal buffer.
    pub fn into_inner(self) -> [CachePadded<T>; N] {
        self.buffer.map(|x| x.into_inner())
    }

    /// Maps `index` to [0, N) for use as an index to the inner buffer.
    fn map_to_valid_index(index: usize) -> usize {
        // Compiler should be able to optimize to `index & (N - 1)` when N is a power of two
        index % N
    }
}

/// Writer half of the `ConveyorBuffer`.
#[repr(transparent)]
pub struct ConveyorBufferWriter<T, const N: usize>(Arc<ConveyorBuffer<T, N>>);

unsafe impl<T, const N: usize> Send for ConveyorBufferWriter<T, N> where T: Send {}

impl<T, const N: usize> ConveyorBufferWriter<T, N> {
    /// Returns the next item to be written to and its index in the internal buffer.
    pub fn get<'a>(&'a mut self) -> (usize, ConveyorBufferWriterItem<'a, T, N>) {
        // Needs to synchronize-with the `store` on ConveyorBufferWriterItem::drop since this
        // might be moved to another thread
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

        let index = ConveyorBuffer::<T, N>::map_to_valid_index(head);
        unsafe {
            let cell = self.0.buffer.get_unchecked(index);
            let item = ConveyorBufferWriterItem {
                item: &mut *cell.get(),
                writer: self,
                next_head: head.wrapping_add(1),
            };
            (index, item)
        }
    }

    /// Returns the internal `ConveyorBuffer`.
    /// 
    /// This forwards to a call to `Arc::into_inner` and will return exactly one `ConveyorBuffer`
    /// for each channel.
    pub fn into_inner(self) -> Option<ConveyorBuffer<T, N>> {
        Arc::into_inner(self.0)
    }

    /// Modify an item on the buffer. Blocks if the buffer is full.
    pub fn write<F, R>(&mut self, mut write_op: F) -> R
    where
        F: FnMut(usize, &mut T) -> R,
    {
        let (index, mut item) = self.get();
        let result = write_op(index, &mut item);
        std::mem::drop(item);
        result
    }
}

/// Represents the item to be written to.
///
/// `Drop`-ing the item passes it to the reader.
pub struct ConveyorBufferWriterItem<'a, T, const N: usize> {
    item: &'a mut CachePadded<T>,
    writer: &'a mut ConveyorBufferWriter<T, N>,
    next_head: usize,
}

impl<'a, T, const N: usize> Drop for ConveyorBufferWriterItem<'a, T, N> {
    fn drop(&mut self) {
        self.writer.0.head.store(self.next_head, Ordering::Release);
    }
}

impl<'a, T, const N: usize> Deref for ConveyorBufferWriterItem<'a, T, N> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.deref()
    }
}

impl<'a, T, const N: usize> DerefMut for ConveyorBufferWriterItem<'a, T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.deref_mut()
    }
}

/// Reader half of the `ConveyorBuffer`.
#[repr(transparent)]
pub struct ConveyorBufferReader<T, const N: usize>(Arc<ConveyorBuffer<T, N>>);

unsafe impl<T, const N: usize> Send for ConveyorBufferReader<T, N> where T: Send {}

impl<T, const N: usize> ConveyorBufferReader<T, N> {
    /// Returns the next item to be read from and its index in the internal buffer.
    pub fn get<'a>(&'a mut self) -> (usize, ConveyorBufferReaderItem<'a, T, N>) {
        // Needs to synchronize-with the `store` on ConveyorBufferReaderItem::drop since this
        // might be moved to another thread
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

        let index = ConveyorBuffer::<T, N>::map_to_valid_index(tail);
        unsafe {
            let cell = self.0.buffer.get_unchecked(index);
            let item = ConveyorBufferReaderItem {
                item: &*cell.get(),
                reader: self,
                next_tail: tail.wrapping_add(1),
            };
            (index, item)
        }
    }

    /// Returns the internal `ConveyorBuffer`.
    /// 
    /// This forwards to a call to `Arc::into_inner` and will return exactly one `ConveyorBuffer`
    /// for each channel.
    pub fn into_inner(self) -> Option<ConveyorBuffer<T, N>> {
        Arc::into_inner(self.0)
    }

    /// Read an item on the buffer. Blocks if the buffer is empty.
    pub fn read<F, R>(&mut self, mut read_op: F) -> R
    where
        F: FnMut(usize, &T) -> R,
    {
        let (index, item) = self.get();
        let result = read_op(index, &item);
        std::mem::drop(item);
        result
    }
}

/// Represents the item to be read from.
///
/// `Drop`-ing the item passes it back to be reused in the writer.
pub struct ConveyorBufferReaderItem<'a, T, const N: usize> {
    item: &'a CachePadded<T>,
    reader: &'a mut ConveyorBufferReader<T, N>,
    next_tail: usize,
}

impl<'a, T, const N: usize> Drop for ConveyorBufferReaderItem<'a, T, N> {
    fn drop(&mut self) {
        self.reader.0.tail.store(self.next_tail, Ordering::Release);
    }
}

impl<'a, T, const N: usize> Deref for ConveyorBufferReaderItem<'a, T, N> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.deref()
    }
}

/// Create a new reader/writer pair. The data structure *may* be more efficient if `N` is a
/// power of two.
pub fn channel<T, const N: usize>(
    buffer: [T; N],
) -> (ConveyorBufferWriter<T, N>, ConveyorBufferReader<T, N>) {
    let conveyor = Arc::new(ConveyorBuffer {
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
        buffer: buffer.map(|x| UnsafeCell::new(CachePadded::new(x))),
    });
    let writer = ConveyorBufferWriter(conveyor.clone());
    let reader = ConveyorBufferReader(conveyor);
    (writer, reader)
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
    fn conveyor_buffer_test() {
        std::thread::scope(|s| {
            const ITERS: i32 = 1000;

            let array = [0; 32];
            let (mut writer, mut reader) = channel(array);

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

#![allow(dead_code)]

mod cache_aligned;
mod ring_buffer;
mod cyclic_buffer;

use std::{cell::UnsafeCell, sync::Arc};
use ring_buffer::WindowBuffer;

/// The sending end of the channel.
pub struct Sender<T, const N: usize> {
    /// Internal `WindowBuffer` that is shared with the `Receiver`
    inner: UnsafeCell<Arc<WindowBuffer<T, N>>>,
}

unsafe impl<T: Send, const N: usize> Send for Sender<T, N> {}

impl<T, const N: usize> Sender<T, N> {
    /// Creates a new `Sender`. To be used by `spsc_channel` function.
    fn new(inner: Arc<WindowBuffer<T, N>>) -> Self {
        Sender {
            inner: UnsafeCell::new(inner),
        }
    }

    /// Sends an item through the channel. Blocks if buffer is full.
    pub fn send(&self, item: T) {
        unsafe { (*self.inner.get()).push(item) };
    }

    /// Signals the receiver to stop.
    pub fn stop(&self) {
        unsafe { (*self.inner.get()).stop() };
    }
}

/// The receiving end of the channel.
pub struct Receiver<T, const N: usize> {
    /// Internal `WindowBuffer` that is shared with the `Sender`
    inner: UnsafeCell<Arc<WindowBuffer<T, N>>>,
}

unsafe impl<T: Send, const N: usize> Send for Receiver<T, N> {}

impl<T, const N: usize> Receiver<T, N> {
    /// Creates a new `Receiver`. To be used by `spsc_channel` function.
    fn new(inner: Arc<WindowBuffer<T, N>>) -> Self {
        Receiver {
            inner: UnsafeCell::new(inner),
        }
    }

    /// Receive an item from the channel. Returns `None` if buffer is currently
    /// empty. Receiving `None`s does not mean the channel has been stopped and
    /// the stop signal should be checked separately.
    pub fn recv(&self) -> Option<T> {
        unsafe { (*self.inner.get()).pop() }
    }

    /// Checks if stop signal has been received.
    pub fn is_stopped(&self) -> bool {
        unsafe { (*self.inner.get()).is_stopped() }
    }
}

/// Creates a pair of sender and receiver that can be used for inter-thread
/// communication.
pub fn spsc_channel<T, const N: usize>() -> (Sender<T, N>, Receiver<T, N>) {
    let a = Arc::new(WindowBuffer::new());
    (Sender::new(a.clone()), Receiver::new(a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn channel_block() {
        let (sender, _receiver) = spsc_channel::<usize, 8>();

        let handle = thread::spawn(move || {
            for i in 0..2000usize {
                print!("{} ", i);
                sender.send(i);
                println!("{}", i);
            }
            sender.stop();
        });
    }

    // #[test]
    // fn channel_pop_on_main_thread() {
    //     let (sender, receiver) = spsc_channel::<usize, 8>();

    //     let handle = thread::spawn(move || {
    //         for i in 0..2000usize {
    //             sender.send(i);
    //         }
    //         sender.stop();
    //     });

    //     for i in 0..2000usize {
    //         let res = loop {
    //             if let Some(val) = receiver.recv() {
    //                 break val;
    //             }
    //             std::thread::yield_now();
    //         };
    //         assert_eq!(res, i);
    //         if i == 1000 {
    //             // wait to get filled in the other thread
    //             thread::sleep(Duration::new(1, 0));
    //         }
    //     }

    //     handle.join().unwrap();
    //     assert_eq!(receiver.is_stopped(), true);
    // }

    // #[test]
    // fn channel_pop_on_other_thread() {
    //     let (sender, receiver) = spsc_channel::<usize, 8>();

    //     let handle = thread::spawn(move || {
    //         for i in 0..2000 {
    //             let res = loop {
    //                 if let Some(val) = receiver.recv() {
    //                     break val;
    //                 }
    //                 std::thread::yield_now();
    //             };
    //             assert_eq!(res, i);
    //         }
    //         let res = receiver.recv();
    //         assert_eq!(res, None);
    //     });

    //     for i in 0..2000 {
    //         sender.send(i);
    //         if i == 1000 {
    //             // wait to be emptied in the other thread
    //             thread::sleep(Duration::new(1, 0));
    //         }
    //     }

    //     handle.join().unwrap();
    // }
}

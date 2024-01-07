use bytes::Bytes;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use webrtc::rtp;

#[cfg(not(test))]
type TrackRemote = webrtc::track::track_remote::TrackRemote;

#[cfg(test)]
type TrackRemote = tests::DummyTrackRemote;

// TODO: Convert to `thiserror`
#[derive(Debug, PartialEq)]
pub enum ReorderBufferError {
    ReadTimeout,
    ReadError,
    UnorderablePacket,
    BufferFull,
}

/// A `TrackRemote` that returns RTP packets in-order.
pub struct BufferedTrackRemote {
    track: Arc<TrackRemote>,
    expected_seq_num: Option<SequenceNumber>,
    packets: BTreeMap<SequenceNumber, rtp::packet::Packet>,
    read_buffer: Vec<u8>,
    read_timeout: Duration,
    max_unordered_packets: usize,
}

impl BufferedTrackRemote {
    /// Create a `BufferedTrackRemote`.
    ///
    /// - The `TrackRemote` that will be provided with reordering capabilities should be passed to
    /// `track`.
    /// - `initial_seq_num` is the initial sequence number of the RTP stream. If `None`, the
    /// initial sequence number will be set to the sequence number of the first received RTP
    /// packet.
    /// - `read_buffer_size` should be set to the expected size in bytes of a received packet. This
    /// is typically >= to the MTU of 1500 bytes.
    /// - `recv` will return `ReorderBufferError::ReadTimeout` if no packets are received within
    /// `read_timeout_millis` milliseconds.
    /// - if the buffer is unable to reorder more than `max_unordered_packets`, `recv` will return
    /// `ReorderBufferError::BufferFull`.
    pub fn new(
        track: Arc<TrackRemote>,
        initial_seq_num: Option<u16>,
        read_buffer_size: usize,
        read_timeout_millis: u64,
        max_unordered_packets: usize,
    ) -> BufferedTrackRemote {
        BufferedTrackRemote {
            track,
            expected_seq_num: initial_seq_num.map(SequenceNumber::new),
            packets: BTreeMap::new(),
            read_buffer: vec![0u8; read_buffer_size],
            read_timeout: Duration::from_millis(read_timeout_millis),
            max_unordered_packets,
        }
    }

    // TODO: The `#[cold]` annotations here were not tested to produce better code.

    #[cold]
    fn read_timeout(&self) -> Result<(Bytes, u32), ReorderBufferError> {
        Err(ReorderBufferError::ReadTimeout)
    }

    #[cold]
    fn read_error(&self) -> Result<(Bytes, u32), ReorderBufferError> {
        Err(ReorderBufferError::ReadError)
    }

    #[cold]
    fn unorderable_packet(&self) -> Result<(Bytes, u32), ReorderBufferError> {
        Err(ReorderBufferError::UnorderablePacket)
    }

    #[cold]
    fn buffer_full(&self) -> Result<(Bytes, u32), ReorderBufferError> {
        Err(ReorderBufferError::BufferFull)
    }

    /// Get the RTP payload in-order.
    #[inline]
    pub async fn recv(&mut self) -> Result<(Bytes, u32), ReorderBufferError> {
        loop {
            if let Some(first_entry) = self.packets.first_entry() {
                // SAFETY:
                // `expected_seq_num` is always initialized on the first packet received and the
                // first packet is never put into the `BTreeMap`.
                let expected_seq_num = unsafe { self.expected_seq_num.as_mut().unwrap_unchecked() };

                if first_entry.key() == expected_seq_num {
                    let packet = first_entry.remove();

                    *expected_seq_num = expected_seq_num.next();
                    return Ok((packet.payload, packet.header.timestamp));
                }
            }

            let track_read =
                tokio::time::timeout(self.read_timeout, self.track.read(&mut self.read_buffer))
                    .await;

            match track_read {
                Err(_) => {
                    return self.read_timeout();
                }
                Ok(Err(_)) => {
                    return self.read_error();
                }
                Ok(Ok((packet, _))) => {
                    let seq_num = SequenceNumber::new(packet.header.sequence_number);

                    let expected_seq_num = match self.expected_seq_num.as_mut() {
                        Some(expected_seq_num) => expected_seq_num,
                        None => {
                            self.expected_seq_num = Some(seq_num);
                            // rustc should be able to optimize out the `unwrap`
                            self.expected_seq_num.as_mut().unwrap()
                        }
                    };

                    match seq_num.cmp(expected_seq_num) {
                        std::cmp::Ordering::Equal if self.packets.is_empty() => {
                            *expected_seq_num = expected_seq_num.next();
                            return Ok((packet.payload, packet.header.timestamp));
                        }

                        std::cmp::Ordering::Less => {
                            return self.unorderable_packet();
                        }

                        // Either:
                        // 1) Received sequence number is greater than the expected, or
                        // 2) Received sequence number is equal to the expected but has some
                        //    saved packets, in which case the packet needs to be pushed to the
                        //    `BTreeMap` to try to empty them on the next loop
                        _ => {
                            // Discard old packet that has the same sequence number if it exists
                            let _ = self.packets.insert(seq_num, packet);

                            if self.packets.len() > self.max_unordered_packets {
                                return self.buffer_full();
                            }

                            continue;
                        }
                    }
                }
            }
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceNumber(u16);

impl SequenceNumber {
    const fn new(seq_num: u16) -> Self {
        Self(seq_num)
    }

    #[inline]
    fn next(&self) -> SequenceNumber {
        SequenceNumber(self.0.wrapping_add(1))
    }

    /// Total ordering from RFC1982.
    #[inline]
    fn cmp_impl(&self, other: &SequenceNumber) -> std::cmp::Ordering {
        const THRESHOLD: u16 = 1 << 15;

        if self.0 == other.0 {
            std::cmp::Ordering::Equal
        } else {
            if self.0 < other.0 {
                if other.0.wrapping_sub(self.0) < THRESHOLD {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            } else {
                if other.0.wrapping_sub(self.0) > THRESHOLD {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Less
                }
            }
        }
    }
}

impl PartialOrd for SequenceNumber {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp_impl(other))
    }
}

impl Ord for SequenceNumber {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_impl(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{Buf, BufMut, BytesMut};
    use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};
    use std::{collections::VecDeque, sync::Mutex};
    use webrtc::interceptor::Attributes;

    const MAX_MTU: usize = 1500;
    const READ_TIMEOUT_MILLIS: u64 = 5000;

    const LARGE_WINDOW: usize = 128;
    const SEQ_NUM_START: u16 = 65500;
    const NUM_PACKETS: u16 = 10000;

    pub struct DummyTrackRemote {
        packets: Mutex<VecDeque<rtp::packet::Packet>>,
    }

    impl DummyTrackRemote {
        fn new(packets: VecDeque<rtp::packet::Packet>) -> DummyTrackRemote {
            DummyTrackRemote {
                packets: Mutex::new(packets),
            }
        }

        pub async fn read(
            &self,
            _b: &mut [u8],
        ) -> Result<(rtp::packet::Packet, Attributes), webrtc::Error> {
            let mut lock = self.packets.lock().unwrap();
            if let Some(packet) = lock.pop_front() {
                Ok((packet, Attributes::default()))
            } else {
                Err(webrtc::Error::ErrUnknownType)
            }
        }
    }

    #[test]
    fn sequence_number_sort_test() {
        let mut seq_nums: Vec<_> = (0..NUM_PACKETS)
            .map(|offset| SequenceNumber(SEQ_NUM_START.wrapping_add(offset)))
            .collect();
        seq_nums.reverse();
        seq_nums.sort();

        for (offset, seq_num) in (0..NUM_PACKETS).zip(&seq_nums) {
            let val = SEQ_NUM_START.wrapping_add(offset);
            assert_eq!(val, seq_num.0);
        }
    }

    async fn reorder_buffer_test<F: FnMut(&mut Vec<SequenceNumber>)>(
        mut f: F,
    ) -> Result<(), ReorderBufferError> {
        let mut seq_nums: Vec<_> = (0..NUM_PACKETS)
            .map(|offset| SequenceNumber(SEQ_NUM_START.wrapping_add(offset)))
            .collect();

        f(&mut seq_nums);

        let packets: VecDeque<_> = seq_nums
            .iter()
            .map(|seq_num| {
                let mut payload = BytesMut::new();
                payload.put_u16(seq_num.0);
                rtp::packet::Packet {
                    header: rtp::header::Header {
                        sequence_number: seq_num.0,
                        ..Default::default()
                    },
                    payload: payload.freeze(),
                }
            })
            .collect();

        seq_nums.sort();

        let track = DummyTrackRemote::new(packets.clone());
        let mut buffered_track = BufferedTrackRemote::new(
            Arc::new(track),
            None,
            MAX_MTU,
            READ_TIMEOUT_MILLIS,
            LARGE_WINDOW,
        );

        let saved_packets_len = buffered_track.packets.len();

        for seq_num in seq_nums {
            let (mut b, _) = buffered_track.recv().await?;
            assert_eq!(seq_num.0, b.get_u16());
        }

        assert_eq!(buffered_track.packets.len(), saved_packets_len);

        Ok(())
    }

    #[tokio::test]
    async fn reorder_buffer_inorder_test() {
        reorder_buffer_test(|_| {}).await.unwrap();
    }

    #[tokio::test]
    async fn reorder_buffer_simple_out_of_order_test() {
        reorder_buffer_test(|seq_nums| {
            // Reorder buffer assumes that the first packet is really the initial packet, so this
            // does not touch index 0
            for i in (2..seq_nums.len()).step_by(2) {
                seq_nums.swap(i, i - 1);
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn reorder_buffer_randomized_test() {
        let mut rng = StdRng::seed_from_u64(0);

        reorder_buffer_test(|seq_nums| {
            // Randomize seq_nums, leaving index 0 alone
            const N: usize = LARGE_WINDOW;
            for start in (1..seq_nums.len()).step_by(N) {
                let end = usize::min(start + N, seq_nums.len());
                seq_nums[start..end].shuffle(&mut rng);
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn reorder_buffer_unorderable_test() {
        assert_eq!(
            reorder_buffer_test(|seq_nums| {
                // - `SEQ_NUM_START` received
                // - Expecting `SEQ_NUM_START + 1`
                // - Received `SEQ_NUM_START` which should come earlier than `SEQ_NUM_START + 1`
                // - return `ReorderBufferError::UnorderablePacket`
                seq_nums[1] = SequenceNumber::new(SEQ_NUM_START);
            })
            .await,
            Err(ReorderBufferError::UnorderablePacket)
        );
    }

    #[tokio::test]
    async fn reorder_buffer_large_window() {
        reorder_buffer_test(|seq_nums| {
            seq_nums.swap(1, LARGE_WINDOW as usize);
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn reorder_buffer_full_test() {
        assert_eq!(
            reorder_buffer_test(|seq_nums| {
                seq_nums.swap(1, (LARGE_WINDOW + 1) as usize);
            })
            .await,
            Err(ReorderBufferError::BufferFull)
        );
    }
}

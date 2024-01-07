use bytes::Bytes;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::time::timeout;
use webrtc::rtp;

const MAX_MTU: usize = 1500;
const READ_TIMEOUT: Duration = Duration::from_millis(5000);

#[cfg(not(test))]
type TrackRemote = webrtc::track::track_remote::TrackRemote;

#[cfg(test)]
type TrackRemote = tests::DummyTrackRemote;

#[derive(Debug)]
pub enum ReorderBufferError {
    HeaderParsingError,
    TrackRemoteReadTimeout,
    TrackRemoteReadError,
    PacketTooShort,
    BufferFull,
    UnorderablePacketReceived,
}

pub struct BufferedTrackRemote {
    track: Arc<TrackRemote>,
    expected_seq_num: Option<SequenceNumber>,
    packets: BTreeMap<SequenceNumber, rtp::packet::Packet>,
}

impl BufferedTrackRemote {
    pub fn new(track: Arc<TrackRemote>, _buffer_size: usize) -> BufferedTrackRemote {
        BufferedTrackRemote {
            track,
            expected_seq_num: None,
            packets: BTreeMap::new(),
        }
    }

    #[cold]
    fn track_read_timeout(&self) -> Result<(Bytes, u32), ReorderBufferError> {
        Err(ReorderBufferError::TrackRemoteReadTimeout)
    }

    #[cold]
    fn track_read_error(&self) -> Result<(Bytes, u32), ReorderBufferError> {
        Err(ReorderBufferError::TrackRemoteReadError)
    }

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

            let mut scratch = [0u8; MAX_MTU];

            let track_read = timeout(READ_TIMEOUT, self.track.read(&mut scratch)).await;
            match track_read {
                Err(_) => {
                    return self.track_read_timeout();
                }
                Ok(Err(_)) => {
                    return self.track_read_error();
                }
                Ok(Ok((packet, _))) => {
                    let seq_num = SequenceNumber(packet.header.sequence_number);

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
                            return Err(ReorderBufferError::UnorderablePacketReceived)
                        }

                        // Either:
                        // 1) Received sequence number is greater than the expected, or
                        // 2) Received sequence number is equal to the expected but has some
                        //    saved packets, in which case the packet needs to be pushed to the
                        //    `BTreeMap` to try to empty them on the next loop
                        _ => {
                            // Discard old packet that has the same sequence number if it exists
                            let _ = self.packets.insert(seq_num, packet);
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
    use std::{collections::VecDeque, sync::Mutex};
    use webrtc::interceptor::Attributes;

    const NUM_PACKETS_TO_BUFFER: usize = 128;

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
    fn sequence_number_sort() {
        const START: u16 = 65500;
        const N: u16 = 10000;
        let mut seq_nums: Vec<_> = (0..N)
            .map(|offset| SequenceNumber(START.wrapping_add(offset)))
            .collect();
        seq_nums.reverse();
        seq_nums.sort();

        for (offset, seq_num) in (0..N).zip(&seq_nums) {
            let val = START.wrapping_add(offset);
            assert_eq!(val, seq_num.0);
        }
    }

    async fn reorder_buffer_test(mut seq_nums: Vec<SequenceNumber>) {
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
        let mut buffered_track = BufferedTrackRemote::new(Arc::new(track), NUM_PACKETS_TO_BUFFER);

        let saved_packets_len = buffered_track.packets.len();

        for seq_num in seq_nums {
            let (mut b, _) = buffered_track.recv().await.unwrap();
            assert_eq!(seq_num.0, b.get_u16());
        }

        assert_eq!(buffered_track.packets.len(), saved_packets_len);
    }

    #[tokio::test]
    async fn reorder_buffer_inorder_test() {
        const START: u16 = 65500;
        const N: u16 = 10000;
        let seq_nums: Vec<_> = (0..N)
            .map(|offset| SequenceNumber(START.wrapping_add(offset)))
            .collect();
        reorder_buffer_test(seq_nums).await;
    }

    #[tokio::test]
    async fn reorder_buffer_simple_out_of_order_test() {
        const START: u16 = 65500;
        const N: u16 = 10000;
        let mut seq_nums: Vec<_> = (0..N)
            .map(|offset| SequenceNumber(START.wrapping_add(offset)))
            .collect();

        // Scramble seq_nums, leaving index 0 alone
        for i in (2..seq_nums.len()).step_by(2) {
            seq_nums.swap(i, i - 1);
        }

        reorder_buffer_test(seq_nums).await;
    }

    #[tokio::test]
    async fn reorder_buffer_large_window() {
        const START: u16 = 65500;
        const N: u16 = 10000;
        let mut seq_nums: Vec<_> = (0..N)
            .map(|offset| SequenceNumber(START.wrapping_add(offset)))
            .collect();

        seq_nums.swap(1, NUM_PACKETS_TO_BUFFER as usize);

        reorder_buffer_test(seq_nums).await;
    }
}

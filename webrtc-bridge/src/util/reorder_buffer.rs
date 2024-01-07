use bytes::Buf;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::time::timeout;
use webrtc::{rtp, util::Unmarshal};

const MAX_MTU: usize = 1500;
const READ_TIMEOUT: Duration = Duration::from_millis(5000);
const MIN_RTP_HEADER_SIZE: usize = 12;

#[cfg(not(test))]
type TrackRemote = webrtc::track::track_remote::TrackRemote;

#[cfg(test)]
type TrackRemote = dyn tests::DummyTrackRemoteTrait;

#[derive(Debug)]
pub enum ReorderBufferError {
    HeaderParsingError,
    TrackRemoteReadTimeout,
    TrackRemoteReadError,
    PacketTooShort,
    BufferFull,
    UnableToMaintainReorderBuffer,
}

pub struct BufferedTrackRemote {
    track: Arc<TrackRemote>,
    expected_seq_num: Option<SequenceNumber>,
    packets: BTreeMap<SequenceNumber, RawPacket>,
    buffers: Vec<PacketBuffer>,
}

impl BufferedTrackRemote {
    pub fn new(track: Arc<TrackRemote>, buffer_size: usize) -> BufferedTrackRemote {
        let buffers = (0..buffer_size).map(|_| PacketBuffer::new()).collect();

        BufferedTrackRemote {
            track,
            expected_seq_num: None,
            packets: BTreeMap::new(),
            buffers,
        }
    }

    #[cold]
    fn track_read_timeout(&self) -> Result<&[u8], ReorderBufferError> {
        Err(ReorderBufferError::TrackRemoteReadTimeout)
    }

    #[cold]
    fn track_read_error(&self) -> Result<&[u8], ReorderBufferError> {
        Err(ReorderBufferError::TrackRemoteReadError)
    }

    // SAFETY:
    // `self.buffers` should not be empty and `len` should be <= `MAX_MTU`
    #[inline]
    unsafe fn last_buffer_payload(&mut self, len: usize) -> Result<&[u8], ReorderBufferError> {
        let last_buffer = self.buffers.last().unwrap_unchecked();
        let mut b: &[u8] = last_buffer.get_unchecked(..len);

        // Unmarshaling the header would move `b` to point to the payload
        if unmarshal_header(&mut b).is_none() {
            return Err(ReorderBufferError::HeaderParsingError);
        };

        return Ok(b);
    }

    #[inline]
    pub async fn recv(&mut self) -> Result<&[u8], ReorderBufferError> {
        loop {
            if let Some(first_entry) = self.packets.first_entry() {
                // SAFETY:
                // `expected_seq_num` is always initialized on the first packet received and the
                // first packet is never put into the `BTreeMap`.
                let expected_seq_num = unsafe { self.expected_seq_num.as_mut().unwrap_unchecked() };

                if first_entry.key() == expected_seq_num {
                    let packet = first_entry.remove();
                    let RawPacket { buffer, len } = packet;

                    // Reuse the buffer, adding it to the last spot
                    self.buffers.push(buffer);

                    // Advance the expected sequence number regardless of errors in the next steps
                    *expected_seq_num = expected_seq_num.next();

                    // SAFETY: A buffer was just pushed and we trust the number of bytes retured
                    // by `TrackRemote::read`
                    return unsafe { self.last_buffer_payload(len) };
                }
            }

            let last_buffer = match self.buffers.last_mut() {
                Some(b) => b,
                None => {
                    if let Some((first_seq_num, _)) = self.packets.first_key_value() {
                        // Force the first entry to be returned next
                        self.expected_seq_num = Some(*first_seq_num);
                        return Err(ReorderBufferError::BufferFull);
                    } else {
                        // `self.buffers.is_empty()` implies `!self.packets.is_empty()`
                        unreachable!()
                    }
                }
            };

            let track_read = timeout(READ_TIMEOUT, self.track.read(last_buffer)).await;
            match track_read {
                Err(_) => {
                    return self.track_read_timeout();
                }
                Ok(Err(_)) => {
                    return self.track_read_error();
                }
                Ok(Ok((len, _))) => {
                    if len < MIN_RTP_HEADER_SIZE {
                        return Err(ReorderBufferError::PacketTooShort);
                    }

                    let seq_num = last_buffer.get_sequence_number();

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
                            // Advance the expected sequence number regardless of errors in the
                            // next steps
                            *expected_seq_num = expected_seq_num.next();

                            // SAFETY: `self.buffers.last_mut()` returned a `Some` and we trust the
                            // number of bytes retured by `TrackRemote::read`
                            return unsafe { self.last_buffer_payload(len) };
                        }

                        std::cmp::Ordering::Less => {
                            return Err(ReorderBufferError::UnableToMaintainReorderBuffer)
                        }

                        // Either:
                        // 1) Received sequence number is greater than the expected, or
                        // 2) Received sequence number is equal to the expected but has some
                        //    saved packets, in which case the packet needs to be pushed to the
                        //    `BTreeMap` to try to empty them on the next loop
                        _ => {
                            let packet = RawPacket {
                                // rustc should be able to optimize out the `unwrap`
                                buffer: self.buffers.pop().unwrap(),
                                len,
                            };
                            if let Some(packet) = self.packets.insert(seq_num, packet) {
                                self.buffers.push(packet.buffer);
                            }
                            continue;
                        }
                    }
                }
            }
        }
    }
}

#[inline]
fn unmarshal_header(buffer: &mut &[u8]) -> Option<rtp::header::Header> {
    // TODO: The header itself is not needed, modify the unmarshal method
    let header = rtp::header::Header::unmarshal(buffer).ok()?;
    if header.padding {
        let payload_len = buffer.remaining();
        if payload_len > 0 {
            let padding_len = buffer[payload_len - 1] as usize;
            if padding_len <= payload_len {
                *buffer = &buffer[..payload_len - padding_len];
                Some(header)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        Some(header)
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

struct PacketBuffer(Box<[u8; MAX_MTU]>);

impl std::ops::Deref for PacketBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        <[u8; MAX_MTU]>::as_slice(&self.0)
    }
}

impl std::ops::DerefMut for PacketBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        <[u8; MAX_MTU]>::as_mut_slice(&mut self.0)
    }
}

impl PacketBuffer {
    fn new() -> PacketBuffer {
        PacketBuffer(TryFrom::try_from(vec![0; MAX_MTU].into_boxed_slice()).unwrap())
    }

    fn get_sequence_number(&self) -> SequenceNumber {
        SequenceNumber(u16::from_be_bytes([self.0[2], self.0[3]]))
    }
}

pub struct RawPacket {
    buffer: PacketBuffer,
    len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{Buf, BufMut, Bytes, BytesMut};
    use std::{
        collections::{HashMap, VecDeque},
        sync::Mutex,
    };
    use webrtc::{
        rtp::{header::Header, packet::Packet},
        util::Marshal,
    };

    const NUM_PACKETS_TO_BUFFER: usize = 128;

    #[async_trait::async_trait]
    pub trait DummyTrackRemoteTrait {
        async fn read(
            &self,
            b: &mut [u8],
        ) -> Result<(usize, std::collections::HashMap<usize, usize>), webrtc::Error>;
    }

    struct DummyTrackRemote {
        packets: Mutex<VecDeque<Bytes>>,
    }

    impl DummyTrackRemote {
        fn new(packets: VecDeque<Bytes>) -> DummyTrackRemote {
            DummyTrackRemote {
                packets: Mutex::new(packets),
            }
        }
    }

    #[async_trait::async_trait]
    impl DummyTrackRemoteTrait for DummyTrackRemote {
        async fn read(
            &self,
            b: &mut [u8],
        ) -> Result<(usize, HashMap<usize, usize>), webrtc::Error> {
            let mut lock = self.packets.lock().unwrap();
            if let Some(packet) = lock.pop_front() {
                let min_len = usize::min(packet.len(), b.len());
                b[..min_len].copy_from_slice(&packet[..min_len]);
                Ok((min_len, HashMap::new()))
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
                let packet = Packet {
                    header: Header {
                        sequence_number: seq_num.0,
                        ..Default::default()
                    },
                    payload: payload.freeze(),
                };
                packet.marshal().unwrap()
            })
            .collect();

        seq_nums.sort();

        let track = DummyTrackRemote::new(packets.clone());
        let mut buffered_track = BufferedTrackRemote::new(Arc::new(track), NUM_PACKETS_TO_BUFFER);

        let buf_len = buffered_track.buffers.len();
        let saved_packets_len = buffered_track.packets.len();

        for seq_num in seq_nums {
            let mut b = buffered_track.recv().await.unwrap();
            assert_eq!(seq_num.0, b.get_u16());
        }

        assert_eq!(buffered_track.buffers.len(), buf_len);
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

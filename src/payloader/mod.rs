//! Modified from rtp::codecs::h264::H264Payloader to allow directly sending the packets without
//! allocating a Vec and annotated infrequently encountered branches with #[cold].

use bytes::{BufMut, Bytes, BytesMut};
use webrtc::{
    rtp::{header::Header, packet::Packet},
    track::track_local::TrackLocalWriter,
};

trait RtpHeaderExt {
    fn advance_sequence_number(&mut self);
}

impl RtpHeaderExt for Header {
    fn advance_sequence_number(&mut self) {
        self.sequence_number = self.sequence_number.wrapping_add(1);
    }
}

/// H264Payloader payloads H264 packets
#[derive(Default, Debug, Clone)]
pub struct H264Payloader {
    sps_nalu: Option<Bytes>,
    pps_nalu: Option<Bytes>,
}

const FUA_NALU_TYPE: u8 = 28;
const SPS_NALU_TYPE: u8 = 7;
const PPS_NALU_TYPE: u8 = 8;
const AUD_NALU_TYPE: u8 = 9;
const FILLER_NALU_TYPE: u8 = 12;

const FUA_HEADER_SIZE: usize = 2;

const NALU_TYPE_BITMASK: u8 = 0x1F;
const NALU_REF_IDC_BITMASK: u8 = 0x60;

const OUTPUT_STAP_AHEADER: u8 = 0x78;

impl H264Payloader {
    fn next_ind(nalu: &Bytes, start: usize) -> (isize, isize) {
        let mut zero_count = 0;

        for (i, &b) in nalu[start..].iter().enumerate() {
            if b == 0 {
                zero_count += 1;
                continue;
            } else if b == 1 && zero_count >= 2 {
                return ((start + i - zero_count) as isize, zero_count as isize + 1);
            }
            zero_count = 0
        }
        (-1, -1)
    }

    #[cold]
    async fn emit_single_nalu<T>(
        header: &mut Header,
        nalu: &Bytes,
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        debug_assert!(nalu.len() <= mtu);
        let mut p = Packet {
            header: header.clone(),
            payload: Bytes::from(nalu.clone()),
        };
        p.header.marker = true;
        header.advance_sequence_number();
        writer.write_rtp(&p).await?;
        return Ok(());
    }

    #[cold]
    async fn emit_parameter_sets<T>(
        header: &mut Header,
        sps_nalu: Bytes,
        pps_nalu: Bytes,
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        // Pack current NALU with SPS and PPS as STAP-A
        let sps_len = (sps_nalu.len() as u16).to_be_bytes();
        let pps_len = (pps_nalu.len() as u16).to_be_bytes();

        let stap_a_nalu_len = 1 + 2 + sps_nalu.len() + 2 + pps_nalu.len();

        if stap_a_nalu_len <= mtu {
            let mut stap_a_nalu = Vec::with_capacity(stap_a_nalu_len);
            stap_a_nalu.push(OUTPUT_STAP_AHEADER);
            stap_a_nalu.extend(sps_len);
            stap_a_nalu.extend_from_slice(&sps_nalu);
            stap_a_nalu.extend(pps_len);
            stap_a_nalu.extend_from_slice(&pps_nalu);

            // FIXME: Marker bit
            let p = Packet {
                header: header.clone(),
                payload: Bytes::from(stap_a_nalu),
            };
            header.advance_sequence_number();
            writer.write_rtp(&p).await?;
        } else {
            if sps_nalu.len() <= mtu {
                Self::emit_single_nalu(header, &sps_nalu, mtu, writer).await?;
            } else {
                Self::emit_fragmented_non_inline(
                    header,
                    sps_nalu[0] & NALU_TYPE_BITMASK,
                    sps_nalu[0] & NALU_REF_IDC_BITMASK,
                    &sps_nalu,
                    mtu,
                    writer,
                )
                .await?;
            }

            if pps_nalu.len() <= mtu {
                Self::emit_single_nalu(header, &pps_nalu, mtu, writer).await?;
            } else {
                Self::emit_fragmented_non_inline(
                    header,
                    pps_nalu[0] & NALU_TYPE_BITMASK,
                    pps_nalu[0] & NALU_REF_IDC_BITMASK,
                    &pps_nalu,
                    mtu,
                    writer,
                )
                .await?;
            }
        }

        Ok(())
    }

    #[inline(always)]
    async fn emit_fragmented<T>(
        header: &mut Header,
        nalu_type: u8,
        nalu_ref_idc: u8,
        nalu: &Bytes,
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        // FU-A
        let max_fragment_size = mtu as isize - FUA_HEADER_SIZE as isize;

        // The FU payload consists of fragments of the payload of the fragmented
        // NAL unit so that if the fragmentation unit payloads of consecutive
        // FUs are sequentially concatenated, the payload of the fragmented NAL
        // unit can be reconstructed.  The NAL unit type octet of the fragmented
        // NAL unit is not included as such in the fragmentation unit payload,
        // 	but rather the information of the NAL unit type octet of the
        // fragmented NAL unit is conveyed in the F and NRI fields of the FU
        // indicator octet of the fragmentation unit and in the type field of
        // the FU header.  An FU payload MAY have any number of octets and MAY
        // be empty.

        let nalu_data = nalu;
        // According to the RFC, the first octet is skipped due to redundant information
        let mut nalu_data_index = 1;
        let nalu_data_length = nalu.len() as isize - nalu_data_index;
        let mut nalu_data_remaining = nalu_data_length;

        if std::cmp::min(max_fragment_size, nalu_data_remaining) <= 0 {
            return Ok(());
        }

        let buf_size = div_ceil(nalu.len(), mtu);

        // This is brought outside the loop to decrease allocation/deallocation.
        let mut out = BytesMut::with_capacity(buf_size);

        while nalu_data_remaining > 0 {
            let current_fragment_size = std::cmp::min(max_fragment_size, nalu_data_remaining);
            // +---------------+
            // |0|1|2|3|4|5|6|7|
            // +-+-+-+-+-+-+-+-+
            // |F|NRI|  Type   |
            // +---------------+
            let b0 = FUA_NALU_TYPE | nalu_ref_idc;
            out.put_u8(b0);

            // +---------------+
            //|0|1|2|3|4|5|6|7|
            //+-+-+-+-+-+-+-+-+
            //|S|E|R|  Type   |
            //+---------------+

            let mut b1 = nalu_type;
            if nalu_data_remaining == nalu_data_length {
                // Set start bit
                b1 |= 1 << 7;
            } else if nalu_data_remaining - current_fragment_size == 0 {
                // Set end bit
                b1 |= 1 << 6;
            }
            out.put_u8(b1);

            out.put(
                &nalu_data
                    [nalu_data_index as usize..(nalu_data_index + current_fragment_size) as usize],
            );

            nalu_data_remaining -= current_fragment_size;
            nalu_data_index += current_fragment_size;

            let mut p = Packet {
                header: header.clone(),
                payload: out.split().freeze(),
            };
            p.header.marker = !(nalu_data_remaining > 0);
            header.advance_sequence_number();
            writer.write_rtp(&p).await?;
        }

        Ok(())
    }

    #[cold]
    async fn emit_fragmented_non_inline<T>(
        header: &mut Header,
        nalu_type: u8,
        nalu_ref_idc: u8,
        nalu: &Bytes,
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        Self::emit_fragmented(header, nalu_type, nalu_ref_idc, nalu, mtu, writer).await
    }

    #[cold]
    async fn process_parameter_sets<T>(
        &mut self,
        header: &mut Header,
        sps_nalu: Option<Bytes>,
        pps_nalu: Option<Bytes>,
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        if let Some(sps_nalu) = sps_nalu {
            self.sps_nalu = Some(sps_nalu);
        }

        if let Some(pps_nalu) = pps_nalu {
            self.pps_nalu = Some(pps_nalu);
        }

        if self.sps_nalu.is_some() && self.pps_nalu.is_some() {
            if let (Some(sps_nalu), Some(pps_nalu)) = (self.sps_nalu.take(), self.pps_nalu.take()) {
                Self::emit_parameter_sets(header, sps_nalu, pps_nalu, mtu, writer).await?;
            } else {
                // `sps_nalu` and `pps_nalu` were already checked using `is_some`
                unreachable!()
            }
        }

        Ok(())
    }

    #[cold]
    fn emit_unhandled_nalu() -> Result<(), webrtc::Error> {
        Ok(())
    }

    async fn emit<T>(
        &mut self,
        header: &mut Header,
        nalu: &Bytes,
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        if nalu.is_empty() {
            return Ok(());
        }

        let nalu_type = nalu[0] & NALU_TYPE_BITMASK;
        let nalu_ref_idc = nalu[0] & NALU_REF_IDC_BITMASK;

        if nalu_type == AUD_NALU_TYPE || nalu_type == FILLER_NALU_TYPE {
            Self::emit_unhandled_nalu()
        } else if nalu_type == SPS_NALU_TYPE {
            self.process_parameter_sets(header, Some(nalu.clone()), None, mtu, writer)
                .await
        } else if nalu_type == PPS_NALU_TYPE {
            self.process_parameter_sets(header, None, Some(nalu.clone()), mtu, writer)
                .await
        } else {
            if nalu.len() <= mtu {
                Self::emit_single_nalu(header, nalu, mtu, writer).await
            } else {
                Self::emit_fragmented(header, nalu_type, nalu_ref_idc, nalu, mtu, writer).await
            }
        }
    }

    /// Payload fragments a H264 packet across one or more byte arrays
    pub async fn write_to_rtp<T>(
        &mut self,
        mtu: usize,
        header: &mut Header,
        payload: &Bytes,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        if payload.is_empty() || mtu == 0 {
            return Ok(());
        }

        header.marker = false;

        let (mut next_ind_start, mut next_ind_len) = H264Payloader::next_ind(payload, 0);
        if next_ind_start == -1 {
            self.emit(header, payload, mtu, writer).await?;
        } else {
            while next_ind_start != -1 {
                let prev_start = (next_ind_start + next_ind_len) as usize;
                let (next_ind_start2, next_ind_len2) = H264Payloader::next_ind(payload, prev_start);
                next_ind_start = next_ind_start2;
                next_ind_len = next_ind_len2;
                if next_ind_start != -1 {
                    self.emit(
                        header,
                        &payload.slice(prev_start..next_ind_start as usize),
                        mtu,
                        writer,
                    )
                    .await?;
                } else {
                    // Emit until end of stream, no end indicator found
                    self.emit(header, &payload.slice(prev_start..), mtu, writer)
                        .await?;
                }
            }
        }
        Ok(())
    }
}

/// Calculate the quotient, rounding up.
///
/// Implementation taken from unstable Rust feature in [`std`][std].
///
/// [std]: https://github.com/rust-lang/rust/blob/b15ca6635f752fefebfd101aa944c6167128183c/library/core/src/num/uint_macros.rs#L2059
const fn div_ceil(lhs: usize, rhs: usize) -> usize {
    let d = lhs / rhs;
    let r = lhs % rhs;
    if r > 0 && rhs > 0 {
        d + 1
    } else {
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        sync::{atomic::AtomicBool, Arc, Mutex},
    };
    use webrtc::{
        media::Sample,
        rtp::{self, header::Header},
        rtp_transceiver::rtp_codec::{
            RTCRtpCodecCapability, RTCRtpCodecParameters, RTCRtpParameters,
        },
        track::track_local::{
            track_local_static_sample::TrackLocalStaticSample, TrackLocal, TrackLocalContext,
        },
        util::{MarshalSize, Unmarshal},
    };

    // const H264_CSD_BYTES: &'static [u8] = include_bytes!("h264-csd.bin");
    // const H264_INBAND_CSD_BYTES: &'static [u8] = include_bytes!("h264-inband-csd.bin");

    #[derive(Debug)]
    struct PacketVec(Mutex<Vec<rtp::packet::Packet>>);

    #[async_trait::async_trait]
    impl TrackLocalWriter for PacketVec {
        async fn write_rtp(&self, p: &rtp::packet::Packet) -> Result<usize, webrtc::Error> {
            match self.0.try_lock() {
                Ok(mut lock) => {
                    lock.push(p.clone());
                    Ok(p.marshal_size())
                }
                Err(_) => Err(webrtc::Error::ErrUnknownType),
            }
        }

        async fn write(&self, mut b: &[u8]) -> Result<usize, webrtc::Error> {
            let pkt = rtp::packet::Packet::unmarshal(&mut b)?;
            self.write_rtp(&pkt).await
        }
    }

    impl PacketVec {
        fn new() -> PacketVec {
            PacketVec(Mutex::new(Vec::new()))
        }

        fn clear(&self) {
            self.0.lock().unwrap().clear();
        }

        fn clone_inner(&self) -> Vec<Packet> {
            self.0.lock().unwrap().clone()
        }
    }

    #[derive(Default)]
    struct FakeTrackLocalContext {
        id: String,
        params: RTCRtpParameters,
        ssrc: u32,
        write_stream: Option<Arc<dyn TrackLocalWriter + Send + Sync>>,
        paused: Arc<AtomicBool>,
    }

    impl FakeTrackLocalContext {
        fn new(
            write_stream: Arc<PacketVec>,
            capability: RTCRtpCodecCapability,
        ) -> FakeTrackLocalContext {
            FakeTrackLocalContext {
                id: "fake-context".to_owned(),
                params: RTCRtpParameters {
                    header_extensions: Vec::new(),
                    codecs: vec![RTCRtpCodecParameters {
                        capability,
                        payload_type: 100,
                        stats_id: "stats_id-0".to_owned(),
                    }],
                },
                ssrc: 0,
                write_stream: Some(write_stream),
                ..Default::default()
            }
        }
    }

    fn create_track_local_context(
        write_stream: Arc<PacketVec>,
        capability: RTCRtpCodecCapability,
    ) -> TrackLocalContext {
        unsafe { std::mem::transmute(FakeTrackLocalContext::new(write_stream, capability)) }
    }

    #[tokio::test]
    async fn test_payloader() {
        let capability = RTCRtpCodecCapability {
            mime_type: "video/H264".to_owned(),
            clock_rate: 90000,
            channels: 0,
            sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=64001f"
                .to_owned(),
            rtcp_feedback: Vec::new(),
        };

        let write_stream = Arc::new(PacketVec::new());
        let t = create_track_local_context(write_stream.clone(), capability.clone());

        let nalus = {
            let mut nalus = Vec::new();
            for i in 0..10 {
                let mut data = Vec::new();

                let file_name = format!("scratch/nalus/{i}.h264");
                let mut file = std::fs::File::open(file_name).unwrap();
                file.read_to_end(&mut data).unwrap();

                nalus.push(data);
            }
            nalus
        };

        {
            let track_local =
                TrackLocalStaticSample::new(capability, "test".to_owned(), "0".to_owned());

            track_local.bind(&t).await.unwrap();

            for nalu in &nalus {
                track_local
                    .write_sample(&Sample {
                        data: Bytes::copy_from_slice(nalu),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
            }
        }

        let reference_packets = write_stream.clone_inner();
        write_stream.clear();

        {
            let mut payloader = H264Payloader::default();

            for nalu in &nalus {
                let payload = Bytes::copy_from_slice(nalu);
                let mut header = Header {
                    version: 2,
                    padding: false,
                    extension: false,
                    marker: false,
                    payload_type: 100,
                    ..Default::default()
                };

                payloader
                    .write_to_rtp(1200 - 12, &mut header, &payload, &*write_stream)
                    .await
                    .unwrap();
            }
        }

        let test_packets = write_stream.clone_inner();

        assert_eq!(test_packets.len(), reference_packets.len());
        for (test, reference) in test_packets.iter().zip(reference_packets) {
            assert_eq!(test.payload, reference.payload);
            assert_eq!(test.header.marker, reference.header.marker);
        }
    }
}

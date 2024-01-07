//! Modified from rtp::codecs::h264::H264Payloader to allow directly sending the packets without
//! allocating a Vec and annotated infrequently encountered branches with #[cold].

use super::{
    super::util::{nalu_chunks, RtpHeaderExt},
    constants::*,
};
use bytes::{BufMut, Bytes, BytesMut};
use webrtc::{
    rtp::{header::Header, packet::Packet},
    track::track_local::TrackLocalWriter,
};

/// `H264SampleSender` payloads H264 packets
#[derive(Default, Debug, Clone)]
pub struct H264SampleSender {
    sps_nalu: Option<Bytes>,
    pps_nalu: Option<Bytes>,
}

impl H264SampleSender {
    #[cold]
    async fn emit_single_nalu<T>(
        header: &mut Header,
        nalu: &[u8],
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        debug_assert!(nalu.len() <= mtu);
        let mut p = Packet {
            header: header.clone(),
            payload: Bytes::copy_from_slice(nalu),
        };
        p.header.marker = true;
        header.advance_sequence_number();
        writer.write_rtp(&p).await?;
        Ok(())
    }

    #[inline(always)]
    async fn emit_fragmented<T>(
        header: &mut Header,
        nalu_type: u8,
        nalu: &[u8],
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
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

        if mtu <= FUA_HEADER_SIZE || nalu.len() <= 1 {
            return Ok(());
        }

        // FU-A
        let max_fragment_size = mtu - FUA_HEADER_SIZE;

        // According to the RFC, the first octet is skipped due to redundant information
        let nalu_data_length = nalu.len() - 1;

        let buf_capacity = {
            let div = nalu_data_length / max_fragment_size;
            let rem = nalu_data_length % max_fragment_size;
            mtu * div + if rem != 0 { 3 + rem } else { 0 }
        };

        // This is brought outside the loop to decrease allocation/deallocation.
        let mut out = BytesMut::with_capacity(buf_capacity);

        // SKip first octet
        let chunks = nalu[1..].chunks(max_fragment_size as usize);
        let end_idx = chunks.len() - 1;

        // +---------------+
        // |0|1|2|3|4|5|6|7|
        // +-+-+-+-+-+-+-+-+
        // |F|NRI|  Type   |
        // +---------------+
        let fu_indicator = (nalu[0] & NALU_REF_IDC_BITMASK) | FUA_NALU_TYPE;

        for (i, chunk) in chunks.enumerate() {
            let fu_header = {
                if i == 0 {
                    1 << 7 | nalu_type // With start bit
                } else if i == end_idx {
                    1 << 6 | nalu_type // With end bit
                } else {
                    nalu_type
                }
            };

            out.put_u8(fu_indicator);
            out.put_u8(fu_header);
            out.put_slice(chunk);

            let mut p = Packet {
                header: header.clone(),
                payload: out.split().freeze(),
            };
            p.header.marker = i == end_idx;
            writer.write_rtp(&p).await?;
            header.advance_sequence_number();
        }

        Ok(())
    }

    #[cold]
    async fn emit_fragmented_non_inline<T>(
        header: &mut Header,
        nalu_type: u8,
        nalu: &[u8],
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        Self::emit_fragmented(header, nalu_type, nalu, mtu, writer).await
    }

    // Don't annotate with `#[cold]` since this is called on only on `process_parameter_sets`
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
        let sps_len = (sps_nalu.len() as u16).to_be_bytes();
        let pps_len = (pps_nalu.len() as u16).to_be_bytes();

        let stap_a_nalu_len = 1 + 2 + sps_nalu.len() + 2 + pps_nalu.len();

        // Try to pack current NALU with SPS and PPS as STAP-A
        if stap_a_nalu_len <= mtu {
            let mut stap_a_nalu = Vec::with_capacity(stap_a_nalu_len);
            stap_a_nalu.push(OUTPUT_STAP_AHEADER);
            stap_a_nalu.extend(sps_len);
            stap_a_nalu.extend_from_slice(&sps_nalu);
            stap_a_nalu.extend(pps_len);
            stap_a_nalu.extend_from_slice(&pps_nalu);

            // TODO: Verify marker bit
            let mut p = Packet {
                header: header.clone(),
                payload: Bytes::from(stap_a_nalu),
            };
            p.header.marker = false;
            header.advance_sequence_number();
            writer.write_rtp(&p).await?;
        } else {
            let nalus = [sps_nalu, pps_nalu];
            for nalu in nalus {
                if nalu.len() <= mtu {
                    Self::emit_single_nalu(header, &nalu, mtu, writer).await?;
                } else {
                    Self::emit_fragmented_non_inline(
                        header,
                        nalu[0] & NALU_TYPE_BITMASK,
                        &nalu,
                        mtu,
                        writer,
                    )
                    .await?;
                }
            }
        }
        Ok(())
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

    #[inline]
    async fn emit<T>(
        &mut self,
        header: &mut Header,
        nalu: &[u8],
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

        if nalu_type == AUD_NALU_TYPE || nalu_type == FILLER_NALU_TYPE {
            Self::emit_unhandled_nalu()
        } else if nalu_type == SPS_NALU_TYPE {
            self.process_parameter_sets(
                header,
                Some(Bytes::copy_from_slice(nalu)),
                None,
                mtu,
                writer,
            )
            .await
        } else if nalu_type == PPS_NALU_TYPE {
            self.process_parameter_sets(
                header,
                None,
                Some(Bytes::copy_from_slice(nalu)),
                mtu,
                writer,
            )
            .await
        } else {
            if nalu.len() <= mtu {
                Self::emit_single_nalu(header, nalu, mtu, writer).await
            } else {
                Self::emit_fragmented(header, nalu_type, nalu, mtu, writer).await
            }
        }
    }

    /// Sends a H264 NALU across one or more byte arrays. The payload must start with a NALU
    /// delimiter (`0b"\x00\x00\x00\x01"`).
    #[inline]
    pub async fn send_payload<T>(
        &mut self,
        mtu: usize,
        header: &mut Header,
        payload: &[u8],
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        if payload.is_empty() || mtu == 0 {
            return Ok(());
        }

        header.marker = false;

        for nalu in nalu_chunks(payload) {
            self.emit(header, nalu, mtu, writer).await?;
        }

        Ok(())
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

    #[allow(dead_code)] // Prevent rust-analyzer from complaining
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
    async fn h264_sender() {
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

                let file_name = format!("src/codecs/h264/nalus/{i}.h264");
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
            let mut sender = H264SampleSender::default();

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

                sender
                    .send_payload(1200 - 12, &mut header, &payload, &*write_stream)
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

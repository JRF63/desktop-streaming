use crate::codecs::{
    h265::constants::*,
    util::{nalu_chunks, RtpHeaderExt},
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use webrtc::{
    rtp::{header::Header, packet::Packet},
    track::track_local::TrackLocalWriter,
};

/// `H265SampleSender` payloads H264 packets
#[derive(Default, Debug, Clone)]
pub struct H265SampleSender {
    vps_nalu: Option<Bytes>,
    sps_nalu: Option<Bytes>,
    pps_nalu: Option<Bytes>,
}

impl H265SampleSender {
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
        writer.write_rtp(&p).await?;
        header.advance_sequence_number();
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
        //  0                   1                   2                   3
        //  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |    PayloadHdr (Type=49)       |   FU header   |               |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+               |
        // |                                                               |
        // |                                                               |
        // |                         FU payload                            |
        // |                                                               |
        // |                               +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |                               :...OPTIONAL RTP padding        |
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

        debug_assert!(mtu > 3);
        debug_assert!(nalu.len() > 2);

        let max_fragment_size = mtu - 3;

        let buf_capacity = {
            let eff_nalu_size = nalu.len() - 2;
            let div = eff_nalu_size / max_fragment_size;
            let rem = eff_nalu_size % max_fragment_size;
            mtu * div + if rem != 0 { 3 + rem } else { 0 }
        };

        let mut out = BytesMut::with_capacity(buf_capacity);

        let payload_header = {
            let mut buf = nalu;
            let nalu_header = buf.get_u16();
            (nalu_header & !NALU_TYPE_MASK) | 49 << 9
        };

        let chunks = nalu[2..].chunks(max_fragment_size);
        let end_idx = chunks.len() - 1;

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

            out.put_u16(payload_header);
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
        vps_nalu: Bytes,
        sps_nalu: Bytes,
        pps_nalu: Bytes,
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        let ap_len = 2 + 2 + vps_nalu.len() + 2 + sps_nalu.len() + 2 + pps_nalu.len();

        // Try to pack VPS/SPS/PPS into one aggregation packet
        if ap_len <= mtu {
            let mut ap_nalu = BytesMut::with_capacity(ap_len);

            let nalu_header: u16 = {
                let headers = {
                    let nalus: [&[u8]; 3] = [&vps_nalu, &sps_nalu, &pps_nalu];
                    nalus.map(|mut nalu| nalu.get_u16())
                };

                // The F bit of the aggregate is 0 if each of the F bits are 0; else it is 1
                let f_bit: u16 = headers
                    .iter()
                    .copied()
                    .reduce(|acc, x| acc | (x & F_BIT_MASK))
                    .unwrap();

                // Lowest LayerId
                let layer_id: u16 = headers
                    .iter()
                    .copied()
                    .reduce(|acc, x| {
                        let layer_id = x & LAYER_ID_MASK;
                        if layer_id < acc {
                            layer_id
                        } else {
                            acc
                        }
                    })
                    .unwrap();

                // Lowest TID
                let tid: u16 = headers
                    .iter()
                    .copied()
                    .reduce(|acc, x| {
                        let tid = x & TID_MASK;
                        if tid < acc {
                            tid
                        } else {
                            acc
                        }
                    })
                    .unwrap();
                f_bit | 48 << 9 | layer_id | tid
            };
            ap_nalu.put_u16(nalu_header);

            ap_nalu.put_u16(vps_nalu.len() as u16);
            ap_nalu.put(vps_nalu);

            ap_nalu.put_u16(sps_nalu.len() as u16);
            ap_nalu.put(sps_nalu);

            ap_nalu.put_u16(pps_nalu.len() as u16);
            ap_nalu.put(pps_nalu);

            let mut p = Packet {
                header: header.clone(),
                payload: ap_nalu.freeze(),
            };
            p.header.marker = false;
            writer.write_rtp(&p).await?;
            header.advance_sequence_number();

        // Send VPS/SPS/PPS one-by-one if they don't fit in one AP
        } else {
            let nalus = [vps_nalu, sps_nalu, pps_nalu];
            for nalu in nalus {
                if nalu.len() <= mtu {
                    Self::emit_single_nalu(header, &nalu, mtu, writer).await?;
                } else {
                    let nalu_type = nalu[0] & TRUNCATED_NALU_TYPE_MASK;
                    Self::emit_fragmented_non_inline(header, nalu_type, &nalu, mtu, writer).await?;
                }
            }
        }
        Ok(())
    }

    #[cold]
    async fn process_parameter_sets<T>(
        &mut self,
        header: &mut Header,
        parameter_set: ParameterSet,
        mtu: usize,
        writer: &T,
    ) -> Result<(), webrtc::Error>
    where
        T: TrackLocalWriter,
    {
        match parameter_set {
            ParameterSet::Vps(b) => self.vps_nalu = Some(b),
            ParameterSet::Sps(b) => self.sps_nalu = Some(b),
            ParameterSet::Pps(b) => self.pps_nalu = Some(b),
        }

        if self.vps_nalu.is_some() && self.sps_nalu.is_some() && self.pps_nalu.is_some() {
            if let (Some(vps_nalu), Some(sps_nalu), Some(pps_nalu)) = (
                self.vps_nalu.take(),
                self.sps_nalu.take(),
                self.pps_nalu.take(),
            ) {
                Self::emit_parameter_sets(header, vps_nalu, sps_nalu, pps_nalu, mtu, writer)
                    .await?;
            } else {
                // `sps_nalu` and `pps_nalu` were already checked using `is_some`
                unreachable!()
            }
        }

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

        let nalu_type = nalu[0] & TRUNCATED_NALU_TYPE_MASK;

        if nalu_type == VPS_NALU_TYPE {
            self.process_parameter_sets(
                header,
                ParameterSet::Vps(Bytes::copy_from_slice(nalu)),
                mtu,
                writer,
            )
            .await
        } else if nalu_type == SPS_NALU_TYPE {
            self.process_parameter_sets(
                header,
                ParameterSet::Sps(Bytes::copy_from_slice(nalu)),
                mtu,
                writer,
            )
            .await
        } else if nalu_type == PPS_NALU_TYPE {
            self.process_parameter_sets(
                header,
                ParameterSet::Pps(Bytes::copy_from_slice(nalu)),
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

    /// Sends an H.265 NALU to the receiver. The payload must start with a NALU delimiter
    /// (`0b"\x00\x00\x00\x01"`).
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

enum ParameterSet {
    Vps(Bytes),
    Sps(Bytes),
    Pps(Bytes),
}

#![allow(dead_code)]
//! Copied from rtp::codecs::h264::H264Payloader to allow reuse of a Vec buffer

use bytes::{BufMut, Bytes, BytesMut};
use webrtc::rtp::{header::Header, packet::Packet};

/// H264Payloader payloads H264 packets
#[derive(Default, Debug, Clone)]
pub struct H264Payloader {
    sps_nalu: Option<Bytes>,
    pps_nalu: Option<Bytes>,
}

pub const STAPA_NALU_TYPE: u8 = 24;
pub const FUA_NALU_TYPE: u8 = 28;
pub const FUB_NALU_TYPE: u8 = 29;
pub const SPS_NALU_TYPE: u8 = 7;
pub const PPS_NALU_TYPE: u8 = 8;
pub const AUD_NALU_TYPE: u8 = 9;
pub const FILLER_NALU_TYPE: u8 = 12;

pub const FUA_HEADER_SIZE: usize = 2;
pub const STAPA_HEADER_SIZE: usize = 1;
pub const STAPA_NALU_LENGTH_SIZE: usize = 2;

pub const NALU_TYPE_BITMASK: u8 = 0x1F;
pub const NALU_REF_IDC_BITMASK: u8 = 0x60;
pub const FU_START_BITMASK: u8 = 0x80;
pub const FU_END_BITMASK: u8 = 0x40;

pub const OUTPUT_STAP_AHEADER: u8 = 0x78;

pub static ANNEXB_NALUSTART_CODE: Bytes = Bytes::from_static(&[0x00, 0x00, 0x00, 0x01]);

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

    fn emit(&mut self, nalu: &Bytes, mtu: usize, packets: &mut Vec<Packet>) {
        if nalu.is_empty() {
            return;
        }

        let nalu_type = nalu[0] & NALU_TYPE_BITMASK;
        let nalu_ref_idc = nalu[0] & NALU_REF_IDC_BITMASK;

        if nalu_type == AUD_NALU_TYPE || nalu_type == FILLER_NALU_TYPE {
            return;
        } else if nalu_type == SPS_NALU_TYPE {
            self.sps_nalu = Some(nalu.clone());
            return;
        } else if nalu_type == PPS_NALU_TYPE {
            self.pps_nalu = Some(nalu.clone());
            return;
        } else if let (Some(sps_nalu), Some(pps_nalu)) = (&self.sps_nalu, &self.pps_nalu) {
            // Pack current NALU with SPS and PPS as STAP-A
            let sps_len = (sps_nalu.len() as u16).to_be_bytes();
            let pps_len = (pps_nalu.len() as u16).to_be_bytes();

            let mut stap_a_nalu = Vec::with_capacity(1 + 2 + sps_nalu.len() + 2 + pps_nalu.len());
            stap_a_nalu.push(OUTPUT_STAP_AHEADER);
            stap_a_nalu.extend(sps_len);
            stap_a_nalu.extend_from_slice(sps_nalu);
            stap_a_nalu.extend(pps_len);
            stap_a_nalu.extend_from_slice(pps_nalu);
            if stap_a_nalu.len() <= mtu {
                packets.push(Packet {
                    header: Header::default(),
                    payload: Bytes::from(stap_a_nalu),
                });
            }
        }

        if self.sps_nalu.is_some() && self.pps_nalu.is_some() {
            self.sps_nalu = None;
            self.pps_nalu = None;
        }

        // Single NALU
        if nalu.len() <= mtu {
            packets.push(Packet {
                header: Header::default(),
                payload: nalu.clone(),
            });
            return;
        }

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
            return;
        }

        while nalu_data_remaining > 0 {
            let current_fragment_size = std::cmp::min(max_fragment_size, nalu_data_remaining);
            //out: = make([]byte, fuaHeaderSize + currentFragmentSize)
            let mut out = BytesMut::with_capacity(FUA_HEADER_SIZE + current_fragment_size as usize);
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
            packets.push(Packet {
                header: Header::default(),
                payload: out.freeze(),
            });

            nalu_data_remaining -= current_fragment_size;
            nalu_data_index += current_fragment_size;
        }
    }

    /// Payload fragments a H264 packet across one or more byte arrays.
    /// Modified from the original to allow reuse of a `Vec`.
    pub fn payload(
        &mut self,
        mtu: usize,
        payload: &Bytes,
        packets: &mut Vec<Packet>,
    ) -> webrtc::error::Result<()> {
        packets.clear();

        if payload.is_empty() || mtu == 0 {
            return Ok(());
        }

        let (mut next_ind_start, mut next_ind_len) = H264Payloader::next_ind(payload, 0);
        if next_ind_start == -1 {
            self.emit(payload, mtu, packets);
        } else {
            while next_ind_start != -1 {
                let prev_start = (next_ind_start + next_ind_len) as usize;
                let (next_ind_start2, next_ind_len2) = H264Payloader::next_ind(payload, prev_start);
                next_ind_start = next_ind_start2;
                next_ind_len = next_ind_len2;
                if next_ind_start != -1 {
                    self.emit(
                        &payload.slice(prev_start..next_ind_start as usize),
                        mtu,
                        packets,
                    );
                } else {
                    // Emit until end of stream, no end indicator found
                    self.emit(&payload.slice(prev_start..), mtu, packets);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H264_CSD_BYTES: &'static [u8] = include_bytes!("h264-csd.bin");
    const H264_INBAND_CSD_BYTES : &'static [u8] = include_bytes!("h264-inband-csd.bin");

    #[test]
    fn csd_fragment() {
        let mut packets = Vec::new();
        let mut payloader = H264Payloader::default();
        payloader
            .payload(1200, &Bytes::from_static(H264_CSD_BYTES), &mut packets)
            .expect("Failed to fragment codec specific data");
    }

    #[test]
    fn inband_csd() {
        let mut packets = Vec::new();
        let mut payloader = H264Payloader::default();
        payloader
            .payload(1200, &Bytes::from_static(H264_INBAND_CSD_BYTES), &mut packets)
            .expect("Failed to fragment codec specific data");
    }
}

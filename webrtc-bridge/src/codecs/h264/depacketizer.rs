use crate::codecs::{
    h264::constants::*,
    util::{Depacketizer, DepacketizerError, UnsafeBufMut},
};

/// `H264Depacketizer` reads payloads from RTP packets and produces NAL units.
pub struct H264Depacketizer<'a> {
    buf_mut: UnsafeBufMut<'a>,
    is_aggregating: bool,
}

impl<'a> Depacketizer for H264Depacketizer<'a> {
    type WrapOutput<'b> = H264Depacketizer<'b>;

    #[inline]
    fn wrap_buffer<'c>(output: &'c mut [u8]) -> Self::WrapOutput<'c> {
        H264Depacketizer {
            buf_mut: UnsafeBufMut::new(output),
            is_aggregating: false,
        }
    }

    #[inline]
    fn push(&mut self, payload: &[u8]) -> Result<(), DepacketizerError> {
        if payload.len() <= FUA_HEADER_SIZE {
            return Err(DepacketizerError::PayloadTooShort);
        }

        // NALU header
        //
        // +---------------+
        // |0|1|2|3|4|5|6|7|
        // +-+-+-+-+-+-+-+-+
        // |F|NRI|  Type   |
        // +---------------+
        let b0 = payload[0];

        // NALU Types
        // https://tools.ietf.org/html/rfc6184#section-5.4
        match b0 & NALU_TYPE_BITMASK {
            1..=23 => H264Depacketizer::single_nalu(self, payload),
            STAPA_NALU_TYPE => H264Depacketizer::stapa_nalu(self, payload),
            FUA_NALU_TYPE => {
                // FU header
                //
                // +---------------+
                // |0|1|2|3|4|5|6|7|
                // +-+-+-+-+-+-+-+-+
                // |S|E|R|  Type   |
                // +---------------+
                let b1 = payload[1];

                if !self.is_aggregating {
                    if b1 & FU_START_BITMASK != 0 {
                        self.is_aggregating = true;

                        let nalu_ref_idc = b0 & NALU_REF_IDC_BITMASK;
                        let fragmented_nalu_type = b1 & NALU_TYPE_BITMASK;

                        if self.buf_mut.remaining_mut() >= ANNEXB_NALUSTART_CODE.len() + 1 {
                            // SAFETY: Checked that the buffer has enough space
                            unsafe {
                                self.buf_mut.put_slice(ANNEXB_NALUSTART_CODE);
                                self.buf_mut.put_u8(nalu_ref_idc | fragmented_nalu_type);
                            }
                        } else {
                            return Err(DepacketizerError::OutputBufferFull);
                        }
                    } else {
                        return Err(DepacketizerError::MissedAggregateStart);
                    }
                }

                // Skip first 2 bytes
                let partial_nalu = &payload[FUA_HEADER_SIZE..];
                if self.buf_mut.remaining_mut() >= partial_nalu.len() {
                    // SAFETY: Checked that the buffer has enough space
                    unsafe {
                        self.buf_mut.put_slice(partial_nalu);
                    }
                } else {
                    return Err(DepacketizerError::OutputBufferFull);
                }

                if b1 & FU_END_BITMASK != 0 {
                    Ok(())
                } else {
                    Err(DepacketizerError::NeedMoreInput)
                }
            }
            _ => H264Depacketizer::other_nalu(self, payload),
        }
    }

    #[inline]
    fn finish(self) -> usize {
        self.buf_mut.num_bytes_written()
    }
}

impl<'a> H264Depacketizer<'a> {
    #[cold]
    fn single_nalu(&mut self, payload: &[u8]) -> Result<(), DepacketizerError> {
        if self.is_aggregating {
            return Err(DepacketizerError::AggregationInterrupted);
        }
        if self.buf_mut.remaining_mut() >= ANNEXB_NALUSTART_CODE.len() + payload.len() {
            // SAFETY: Checked that the buffer has enough space
            unsafe {
                self.buf_mut.put_slice(ANNEXB_NALUSTART_CODE);
                self.buf_mut.put_slice(payload);
            }
            Ok(())
        } else {
            Err(DepacketizerError::OutputBufferFull)
        }
    }

    #[cold]
    fn stapa_nalu(&mut self, payload: &[u8]) -> Result<(), DepacketizerError> {
        if self.is_aggregating {
            return Err(DepacketizerError::AggregationInterrupted);
        }
        let mut curr_offset = STAPA_HEADER_SIZE;

        while curr_offset < payload.len() {
            // Get 2 bytes of the NALU size
            let nalu_size_bytes = payload
                .get(curr_offset..curr_offset + 2)
                .ok_or(DepacketizerError::PayloadTooShort)?;

            // NALU size is a 16-bit unsigned integer in network byte order.
            // The compiler should be able to deduce that `try_into().unwrap()` would not panic.
            let nalu_size = u16::from_be_bytes(nalu_size_bytes.try_into().unwrap()) as usize;

            curr_offset += STAPA_NALU_LENGTH_SIZE;

            let nalu = payload
                .get(curr_offset..curr_offset + nalu_size)
                .ok_or(DepacketizerError::PayloadTooShort)?;

            if self.buf_mut.remaining_mut() >= ANNEXB_NALUSTART_CODE.len() + nalu.len() {
                // SAFETY: Checked that the buffer has enough space
                unsafe {
                    self.buf_mut.put_slice(ANNEXB_NALUSTART_CODE);
                    self.buf_mut.put_slice(nalu);
                }
            } else {
                return Err(DepacketizerError::OutputBufferFull);
            }

            curr_offset += nalu_size;
        }

        Ok(())
    }

    #[cold]
    fn other_nalu(&self, _payload: &[u8]) -> Result<(), DepacketizerError> {
        Err(DepacketizerError::UnsupportedPayloadType)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use webrtc::rtp::{codecs::h264::H264Payloader, packetizer::Payloader};

    const TEST_NALU: &[u8] = include_bytes!("nalus/1.h264");

    #[test]
    fn fragment_then_unfragment() {
        let mut payloader = H264Payloader::default();
        let payloads = payloader
            .payload(1188, &Bytes::copy_from_slice(TEST_NALU))
            .unwrap();

        let mut output = vec![0u8; TEST_NALU.len()];
        let mut reader = H264Depacketizer::wrap_buffer(&mut output);
        let mut bytes_written = None;
        for payload in payloads {
            match reader.push(&payload) {
                Ok(()) => {
                    bytes_written = Some(reader.finish());
                    break;
                }
                Err(DepacketizerError::NeedMoreInput) => continue,
                Err(_) => panic!("Error processing payloads"),
            }
        }

        let n = bytes_written.unwrap();
        assert_eq!(&output[..n], TEST_NALU);
    }
}

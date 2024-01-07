use crate::codecs::{
    h265::constants::*,
    util::{Depacketizer, DepacketizerError, UnsafeBufMut},
};

/// An H.265 depacketizer. This implementation can only handle payloads without the optional DONL.
pub struct H265Depacketizer<'a> {
    buf_mut: UnsafeBufMut<'a>,
    is_aggregating: bool,
}

impl<'a> Depacketizer for H265Depacketizer<'a> {
    type WrapOutput<'b> = H265Depacketizer<'b>;

    #[inline]
    fn wrap_buffer<'c>(output: &'c mut [u8]) -> Self::WrapOutput<'c> {
        H265Depacketizer {
            buf_mut: UnsafeBufMut::new(output),
            is_aggregating: false,
        }
    }

    #[inline]
    fn push(&mut self, payload: &[u8]) -> Result<(), DepacketizerError> {
        // Payload Header
        //
        // +---------------+---------------+
        // |0|1|2|3|4|5|6|7|0|1|2|3|4|5|6|7|
        // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        // |F|   Type    |  LayerId  | TID |
        // +-------------+-----------------+

        if payload.len() <= PAYLOAD_HEADER_SIZE {
            return Err(DepacketizerError::PayloadTooShort);
        }

        match payload[0] & TRUNCATED_NALU_TYPE_MASK {
            0..=47 => self.single_nalu(payload),
            AP_PAYLOAD_TYPE => self.aggregation_packet(payload),
            FU_PAYLOAD_TYPE => {
                // FU Header
                //
                // +---------------+
                // |0|1|2|3|4|5|6|7|
                // +-+-+-+-+-+-+-+-+
                // |S|E|  FuType   |
                // +---------------+
                let fu_header = payload[2];

                if !self.is_aggregating {
                    if fu_header & FU_START_MASK != 0 {
                        self.is_aggregating = true;

                        let payload_header = u16::from_be_bytes([payload[0], payload[1]]);
                        let nalu_header = payload_header & (!NALU_TYPE_MASK);
                        let fragmented_nalu_type =
                            u16::from_be_bytes([fu_header & FU_TYPE_MASK, 0]);

                        if self.buf_mut.remaining_mut()
                            >= NALU_DELIMITER.len() + PAYLOAD_HEADER_SIZE
                        {
                            // SAFETY: Checked that the buffer has enough space
                            unsafe {
                                self.buf_mut.put_slice(NALU_DELIMITER);
                                self.buf_mut.put_u16(nalu_header | fragmented_nalu_type);
                            }
                        } else {
                            return Err(DepacketizerError::OutputBufferFull);
                        }
                    } else {
                        return Err(DepacketizerError::MissedAggregateStart);
                    }
                }

                // Skip first 3 bytes
                let partial_nalu = &payload[(PAYLOAD_HEADER_SIZE + 1)..];
                if self.buf_mut.remaining_mut() >= partial_nalu.len() {
                    // SAFETY: Checked that the buffer has enough space
                    unsafe {
                        self.buf_mut.put_slice(partial_nalu);
                    }
                } else {
                    return Err(DepacketizerError::OutputBufferFull);
                }

                if fu_header & FU_END_MASK != 0 {
                    Ok(())
                } else {
                    Err(DepacketizerError::NeedMoreInput)
                }
            }
            _ => self.other_nalu(payload),
        }
    }

    #[inline]
    fn finish(self) -> usize {
        self.buf_mut.num_bytes_written()
    }
}

impl<'a> H265Depacketizer<'a> {
    #[cold]
    fn single_nalu(&mut self, payload: &[u8]) -> Result<(), DepacketizerError> {
        if self.is_aggregating {
            return Err(DepacketizerError::AggregationInterrupted);
        }
        if self.buf_mut.remaining_mut() >= NALU_DELIMITER.len() + payload.len() {
            // SAFETY: Checked that the buffer has enough space
            unsafe {
                self.buf_mut.put_slice(NALU_DELIMITER);
                self.buf_mut.put_slice(payload);
            }
            Ok(())
        } else {
            Err(DepacketizerError::OutputBufferFull)
        }
    }

    #[cold]
    fn aggregation_packet(&mut self, payload: &[u8]) -> Result<(), DepacketizerError> {
        if self.is_aggregating {
            return Err(DepacketizerError::AggregationInterrupted);
        }
        let mut curr_offset = PAYLOAD_HEADER_SIZE;

        while curr_offset < payload.len() {
            // Get 2 bytes of the NALU size
            let nalu_size_bytes = payload
                .get(curr_offset..curr_offset + 2)
                .ok_or(DepacketizerError::PayloadTooShort)?;

            // NALU size is a 16-bit unsigned integer in network byte order.
            // The compiler should be able to deduce that `try_into().unwrap()` would not panic.
            let nalu_size = u16::from_be_bytes(nalu_size_bytes.try_into().unwrap()) as usize;

            curr_offset += 2;

            let nalu = payload
                .get(curr_offset..curr_offset + nalu_size)
                .ok_or(DepacketizerError::PayloadTooShort)?;

            if self.buf_mut.remaining_mut() >= NALU_DELIMITER.len() + nalu.len() {
                // SAFETY: Checked that the buffer has enough space
                unsafe {
                    self.buf_mut.put_slice(NALU_DELIMITER);
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

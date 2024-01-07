/// Errors that `Depacketizer` can return.
///
/// `NeedMoreInput` is non-fatal and must be treated as a request for more payload.
#[derive(Debug)]
pub enum DepacketizerError {
    NeedMoreInput,
    PayloadTooShort,
    OutputBufferFull,
    UnsupportedPayloadType,
    AggregationInterrupted,
    MissedAggregateStart,
}

/// Depacketizes payloads into a codec specific format (i.e, a NALU).
///
/// This is different from `webrtc::rtp::packetizer::Depacketizer` in that it requires a buffer to
/// be passed for initialization. This is done to prevent unnecessary allocation/deallocation and
/// copying.
pub trait Depacketizer {
    type WrapOutput<'a>: Depacketizer;

    /// Create a new `Depacketizer` by wrapping an existing buffer.
    fn wrap_buffer<'a>(output: &'a mut [u8]) -> Self::WrapOutput<'a>;

    /// Add a payload to be depacketized. This method can return `DepacketizerError::NeedMoreInput`
    /// signaling that the depacketizer needs more packets to complete the data.
    ///
    /// The `Depacketizer` trait assumes that the given payloads are in-order.
    fn push(&mut self, payload: &[u8]) -> Result<(), DepacketizerError>;

    /// Consume the `Depacketizer` and return the number of bytes written.
    fn finish(self) -> usize;
}

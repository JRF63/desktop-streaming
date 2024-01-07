use crate::{codecs::CodecType, Codec, WebRtcPeer};
use std::sync::Arc;
use webrtc::{
    rtp_transceiver::{rtp_codec::RTCRtpCodecCapability, rtp_receiver::RTCRtpReceiver},
    track::track_remote::TrackRemote,
};

/// Encapsulates a builder that produces a decoder.
pub trait DecoderBuilder: Send {
    /// Lists all the supported codecs of the decoder.
    fn supported_codecs(&self) -> &[Codec];

    /// Whether the builder is for an audio or video codec.
    fn codec_type(&self) -> CodecType;

    /// Consumes the builder to produce a decoder.
    ///
    /// Data from the encoder is received through `track` while the `rtp_receiver` is used to send
    /// RTCP messages. The chosen codec is identified through `TrackRemote::codec` of `track`.
    ///
    /// This function will be invoked inside a Tokio runtime such that implementations could assume
    /// that `tokio::runtime::Handle` would not panic.
    fn build(
        self: Box<Self>,
        track: Arc<TrackRemote>,
        rtp_receiver: Arc<RTCRtpReceiver>,
        peer: Arc<WebRtcPeer>,
    );

    /// Checks if the decoder supports the given codec parameters.
    fn is_codec_supported(&self, codec_capability: &RTCRtpCodecCapability) -> bool {
        for supported_codec in self.supported_codecs() {
            if supported_codec.capability_matches(codec_capability) {
                return true;
            }
        }
        false
    }
}

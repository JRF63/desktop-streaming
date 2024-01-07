mod track;

pub use self::track::EncoderTrackLocal;
use crate::{
    codecs::{Codec, CodecType},
    interceptor::twcc::TwccBandwidthEstimate,
    peer::IceConnectionState,
};
use std::sync::Arc;
use webrtc::{
    rtp_transceiver::{rtp_codec::RTCRtpCodecCapability, RTCRtpTransceiver},
    track::track_local::track_local_static_rtp::TrackLocalStaticRTP,
};

/// Encapsulates a builder that produces an encoder.
pub trait EncoderBuilder: Send {
    /// Unique identifier for the track. Used in the `TrackLocal` implementation.
    fn id(&self) -> &str;

    /// Group this track belongs to. Used in the `TrackLocal` implementation.
    fn stream_id(&self) -> &str;

    /// Whether the builder is for an audio or video codec.
    ///
    /// This is required because webrtc-rs rejects the transceiver if
    /// [RTPCodecType::Unspecified][a] is returned by [TrackLocal::kind][b].
    ///
    /// [a]: webrtc::rtp_transceiver::rtp_codec::RTPCodecType::Unspecified
    /// [b]: webrtc::track::track_local::TrackLocal::kind
    fn codec_type(&self) -> CodecType;

    /// List of codecs that the encoder supports.
    fn supported_codecs(&self) -> &[Codec];

    /// Build an encoder given the codec parameters. This function will be invoked inside a
    /// Tokio runtime such that implementations could assume that `tokio::runtime::Handle` would
    /// not panic.
    ///
    /// Encoded samples are to be sent through the `RTCRtpTransceiver`.
    ///
    /// Implementations need to wait for ICE to be connected via `ice_connection_state` before
    /// sending data. The chosen codec is found through `codec_capability`.
    fn build(
        self: Box<Self>,
        rtp_track: Arc<TrackLocalStaticRTP>,
        transceiver: Arc<RTCRtpTransceiver>,
        ice_connection_state: IceConnectionState,
        bandwidth_estimate: TwccBandwidthEstimate,
        codec_capability: RTCRtpCodecCapability,
        ssrc: u32,
        payload_type: u8,
    );

    /// Checks if the encoder supports the given codec parameters.
    fn is_codec_supported(&self, codec_capability: &RTCRtpCodecCapability) -> bool {
        for supported_codec in self.supported_codecs() {
            if supported_codec.capability_matches(codec_capability) {
                return true;
            }
        }
        false
    }
}

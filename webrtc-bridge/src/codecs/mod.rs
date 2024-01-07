pub mod av1;
pub mod h264;
pub mod h265;
pub mod util;

use webrtc::{
    api::media_engine::MediaEngine,
    rtp_transceiver::{
        rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType},
        RTCPFeedback,
    },
};

const MIME_TYPE_H264: &str = "video/H264";
const MIME_TYPE_OPUS: &str = "audio/opus";

// TODO H265:
// See https://www.rfc-editor.org/rfc/rfc7798#section-7.1
// const MIME_TYPE_H265: &str = "video/H265";

/// The type of a [Codec].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CodecType {
    Audio = 1,
    Video = 2,
}

impl Into<RTPCodecType> for CodecType {
    fn into(self) -> RTPCodecType {
        match self {
            CodecType::Audio => RTPCodecType::Audio,
            CodecType::Video => RTPCodecType::Video,
        }
    }
}

/// Abstraction of a media codec for registering in a [MediaEngine].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Codec {
    parameters: RTCRtpCodecParameters,
    codec_type: CodecType,
}

impl Codec {
    /// Create a new [Codec].
    pub fn new(parameters: RTCRtpCodecParameters, codec_type: CodecType) -> Codec {
        Codec {
            parameters,
            codec_type,
        }
    }

    /// Returns the type (audio/video) of the [Codec].
    pub fn codec_type(&self) -> CodecType {
        self.codec_type
    }

    /// Modifies the payload type of the [Codec].
    pub fn set_payload_type(&mut self, payload_type: u8) {
        self.parameters.payload_type = payload_type;
    }

    /// Checks if the [Codec] has the same [RTCRtpCodecCapability] as `codec_capability`.
    pub fn capability_matches(&self, codec_capability: &RTCRtpCodecCapability) -> bool {
        // All parameters except `rtcp_feedback` should match
        let codec_matches = |a: &RTCRtpCodecCapability, b: &RTCRtpCodecCapability| {
            a.mime_type == b.mime_type
                && a.clock_rate == b.clock_rate
                && a.channels == b.channels
                && a.sdp_fmtp_line == b.sdp_fmtp_line
        };

        codec_matches(&self.parameters.capability, codec_capability)
    }

    /// Create an [RFC4588][RFC4588] retransmission [Codec] from a base video [Codec]. Returns
    /// [None] if `base_codec` is of type [CodecType::Audio].
    ///
    /// [RFC4588]: https://www.rfc-editor.org/rfc/rfc4588
    pub fn retransmission(base_codec: &Codec) -> Option<Codec> {
        if base_codec.codec_type() == CodecType::Audio {
            return None;
        }

        let parameters = RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: "video/rtx".to_owned(),
                sdp_fmtp_line: format!("apt={}", base_codec.parameters.payload_type),
                rtcp_feedback: Vec::new(),
                ..base_codec.parameters.capability.clone()
            },
            ..Default::default()
        };

        Some(Codec::new(parameters, CodecType::Video))
    }

    /// Create an [RFC5109][RFC5109] [Codec].
    ///
    /// [RFC5109]: https://www.rfc-editor.org/rfc/rfc5109
    pub fn ulpfec() -> Codec {
        let parameters = RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: "video/ulpfec".to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: Vec::new(),
            },
            ..Default::default()
        };
        Codec::new(parameters, CodecType::Video)
    }

    /// Create an Opus [Codec].
    pub fn opus() -> Codec {
        let parameters = RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: 48000,
                channels: 2,
                sdp_fmtp_line: "minptime=10;useinbandfec=1".to_owned(),
                rtcp_feedback: Vec::new(),
            },
            payload_type: 0,
            ..Default::default()
        };
        Codec::new(parameters, CodecType::Audio)
    }
}

/// RTCP feedbacks that can be handled either by this crate or natively by webrtc-rs.
pub(crate) fn supported_video_rtcp_feedbacks() -> Vec<RTCPFeedback> {
    vec![RTCPFeedback {
        typ: "ccm".to_owned(),
        parameter: "fir".to_owned(),
    }]
}

/// Helper trait for adding methods to [MediaEngine].
pub(crate) trait MediaEngineExt {
    /// Register the [Codec] with a dynamically chose payload type.
    fn register_custom_codec(&mut self, codec: Codec) -> Result<(), webrtc::Error>;
}

impl MediaEngineExt for MediaEngine {
    fn register_custom_codec(&mut self, codec: Codec) -> Result<(), webrtc::Error> {
        self.register_codec(codec.parameters, codec.codec_type.into())?;
        Ok(())
    }
}

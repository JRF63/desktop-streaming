use super::EncoderBuilder;
use crate::{codecs::Codec, interceptor::twcc::TwccBandwidthEstimate, peer::IceConnectionState};
use async_trait::async_trait;
use std::{any::Any, fmt::Debug, sync::Arc};
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Mutex,
};
use webrtc::{
    peer_connection::RTCPeerConnection,
    rtp_transceiver::{
        rtp_codec::{RTCRtpCodecParameters, RTPCodecType},
        rtp_transceiver_direction::RTCRtpTransceiverDirection,
        RTCRtpTransceiver, RTCRtpTransceiverInit,
    },
    track::track_local::{
        track_local_static_rtp::TrackLocalStaticRTP, TrackLocal, TrackLocalContext,
    },
    Error,
};

pub struct EncoderTrackLocal {
    tx: UnboundedSender<TrackLocalEvent>,
    rtp_track: Mutex<Option<Arc<TrackLocalStaticRTP>>>,
    supported_codecs: Vec<Codec>,
    id: String,
    stream_id: String,
    kind: RTPCodecType,
}

#[async_trait]
impl TrackLocal for EncoderTrackLocal {
    async fn bind(&self, t: &TrackLocalContext) -> Result<RTCRtpCodecParameters, webrtc::Error> {
        let mut data = self.rtp_track.lock().await;
        match &mut *data {
            Some(rtp_track) => rtp_track.bind(t).await,
            None => {
                for codec_params in t.codec_parameters() {
                    for codec in &self.supported_codecs {
                        if codec.capability_matches(&codec_params.capability) {
                            let rtp_track = Arc::new(TrackLocalStaticRTP::new(
                                codec_params.capability.clone(),
                                self.id.clone(),
                                self.stream_id.clone(),
                            ));

                            let rtp_params = (t.ssrc(), codec_params.payload_type);
                            self.tx
                                .send(TrackLocalEvent::RtpTrack(rtp_track.clone(), rtp_params))
                                .expect("Error while sending TrackLocalStaticRTP");

                            let bind_result = rtp_track.bind(t).await;
                            let mut new_data = Some(rtp_track);
                            std::mem::swap(&mut *data, &mut new_data);

                            return bind_result;
                        }
                    }
                }
                Err(Error::ErrUnsupportedCodec)
            }
        }
    }

    async fn unbind(&self, t: &TrackLocalContext) -> Result<(), webrtc::Error> {
        match &mut *self.rtp_track.lock().await {
            Some(rtp_track) => rtp_track.unbind(t).await,
            None => Err(Error::ErrUnbindFailed),
        }
    }

    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn stream_id(&self) -> &str {
        self.stream_id.as_str()
    }

    fn kind(&self) -> RTPCodecType {
        self.kind
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EncoderTrackLocal {
    pub async fn new(
        encoder_builder: Box<dyn EncoderBuilder>,
        ice_connection_state: IceConnectionState,
        bandwidth_estimate: TwccBandwidthEstimate,
    ) -> EncoderTrackLocal {
        let id = encoder_builder.id().to_owned();
        let stream_id = encoder_builder.stream_id().to_owned();
        let kind = encoder_builder.codec_type().into();
        let supported_codecs = encoder_builder.supported_codecs().to_vec();

        let (tx, rx) = unbounded_channel();

        tokio::spawn(async move {
            pending_builder(
                rx,
                encoder_builder,
                ice_connection_state,
                bandwidth_estimate,
            )
            .await;
        });

        EncoderTrackLocal {
            tx,
            rtp_track: Mutex::new(None),
            supported_codecs,
            id,
            stream_id,
            kind,
        }
    }

    pub async fn add_as_transceiver(
        self: Arc<EncoderTrackLocal>,
        pc: &RTCPeerConnection,
    ) -> Result<(), webrtc::Error> {
        let transceiver = pc
            .add_transceiver_from_track(
                self.clone(),
                Some(RTCRtpTransceiverInit {
                    direction: RTCRtpTransceiverDirection::Sendonly,
                    send_encodings: Vec::new(),
                }),
            )
            .await?;
        self.tx
            .send(TrackLocalEvent::RtpTransceiver(transceiver))
            .expect("Error while sending RTCRtpTransceiver");
        Ok(())
    }
}

enum TrackLocalEvent {
    RtpTrack(Arc<TrackLocalStaticRTP>, (u32, u8)),
    RtpTransceiver(Arc<RTCRtpTransceiver>),
}

impl Debug for TrackLocalEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RtpTrack(arg0, arg1) => {
                f.debug_tuple("RtpTrack").field(arg0).field(arg1).finish()
            }
            Self::RtpTransceiver(_) => f.debug_tuple("RtpTransceiver").finish(),
        }
    }
}

async fn pending_builder(
    mut rx: UnboundedReceiver<TrackLocalEvent>,
    encoder_builder: Box<dyn EncoderBuilder>,
    ice_connection_state: IceConnectionState,
    bandwidth_estimate: TwccBandwidthEstimate,
) {
    let mut rtp_track: Option<Arc<TrackLocalStaticRTP>> = None;
    let mut transceiver: Option<Arc<RTCRtpTransceiver>> = None;
    let mut rtp_params: Option<(u32, u8)> = None;

    loop {
        if rtp_track.is_some() && transceiver.is_some() && rtp_params.is_some() {
            let rtp_track = rtp_track.unwrap();
            let codec_capability = rtp_track.codec();
            let transceiver = transceiver.unwrap();
            let (ssrc, payload_type) = rtp_params.unwrap();

            encoder_builder.build(
                rtp_track,
                transceiver,
                ice_connection_state,
                bandwidth_estimate,
                codec_capability,
                ssrc,
                payload_type,
            );
            break;
        } else {
            match rx.recv().await {
                Some(event) => match event {
                    TrackLocalEvent::RtpTrack(t, p) => {
                        rtp_track = Some(t);
                        rtp_params = Some(p);
                    }
                    TrackLocalEvent::RtpTransceiver(r) => transceiver = Some(r),
                },
                None => {
                    // TODO: Log error
                    break;
                }
            }
        }
    }
}

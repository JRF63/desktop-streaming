use crate::{
    codecs::{h264::H264Codec, Codec, MediaEngineExt},
    decoder::DecoderBuilder,
    encoder::{EncoderBuilder, EncoderTrackLocal},
    interceptor::{configure_custom_twcc_sender, twcc::TwccBandwidthEstimate},
    network::data_rate::DataRate,
    signaling::{Message, Signaler},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::{watch, Mutex, Notify};
use webrtc::{
    api::{
        interceptor_registry::{
            configure_nack, configure_rtcp_reports, configure_twcc, configure_twcc_receiver_only,
        },
        media_engine::MediaEngine,
        setting_engine::SettingEngine,
        APIBuilder,
    },
    ice::mdns::MulticastDnsMode,
    ice_transport::{ice_connection_state::RTCIceConnectionState, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, offer_answer_options::RTCOfferOptions,
        peer_connection_state::RTCPeerConnectionState, sdp::sdp_type::RTCSdpType,
        signaling_state::RTCSignalingState, OnDataChannelHdlrFn, RTCPeerConnection,
    },
    rtp_transceiver::{
        rtp_receiver::RTCRtpReceiver, rtp_transceiver_direction::RTCRtpTransceiverDirection,
        RTCRtpTransceiver, RTCRtpTransceiverInit,
    },
    track::track_remote::TrackRemote,
};

/// Used for querying `RTCIceConnectionState` in the encoders/decoders.
pub type IceConnectionState = watch::Receiver<RTCIceConnectionState>;

/// Determines if the peer will offer or wait for an SDP.
///
/// The role of each peer needs to be specified at the start since the `webrtc` crate does not
/// support any form of rollback and cannot use ["perfect negotiation"][PN].
///
/// [PN]: https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Perfect_negotiation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Offerer,
    Answerer,
}

/// Builder for a `WebRtcPeer`.
pub struct WebRtcBuilder<S>
where
    S: Signaler + 'static,
{
    signaler: S,
    role: Role,
    ice_servers: Vec<RTCIceServer>,
    encoders: Vec<Box<dyn EncoderBuilder>>,
    decoders: Vec<Box<dyn DecoderBuilder>>,
    data_channel_handler: Option<OnDataChannelHdlrFn>,
    init_bandwidth: DataRate,
}

impl<S> WebRtcBuilder<S>
where
    S: Signaler + 'static,
{
    /// Create a new `WebRtcBuilder`.
    pub fn new(signaler: S, role: Role) -> Self {
        WebRtcBuilder {
            signaler,
            role,
            ice_servers: Vec::new(),
            encoders: Vec::new(),
            decoders: Vec::new(),
            data_channel_handler: None,
            init_bandwidth: DataRate::from_bits_per_sec(1_000_000), // 1 Mbps
        }
    }

    /// Add an encoder.
    pub fn with_encoder(&mut self, encoder: Box<dyn EncoderBuilder>) -> &mut Self {
        self.encoders.push(encoder);
        self
    }

    /// Add a decoder.
    pub fn with_decoder(&mut self, decoder: Box<dyn DecoderBuilder>) -> &mut Self {
        self.decoders.push(decoder);
        self
    }

    /// Build using the given ICE servers.
    pub fn with_ice_servers(&mut self, ice_servers: &[RTCIceServer]) -> &mut Self {
        self.ice_servers.clear();
        self.ice_servers.extend_from_slice(ice_servers);
        self
    }

    /// Add a callback for sending/receiving data through a [RTCDataChannel][dc].
    ///
    /// [dc]: webrtc::data_channel::RTCDataChannel
    pub fn with_data_channel_handler(
        &mut self,
        data_channel_handler: OnDataChannelHdlrFn,
    ) -> &mut Self {
        self.data_channel_handler = Some(data_channel_handler);
        self
    }

    pub fn initial_bandwidth(&mut self, init_bandwidth: DataRate) -> &mut Self {
        self.init_bandwidth = init_bandwidth;
        self
    }

    /// Consume the builder and build a `WebRtcPeer`.
    pub async fn build(self) -> webrtc::error::Result<Arc<WebRtcPeer>> {
        let mut media_engine = MediaEngine::default();
        {
            let mut codecs = Vec::new();
            for encoder in self.encoders.iter() {
                codecs.extend_from_slice(encoder.supported_codecs());
            }
            for decoder in self.decoders.iter() {
                codecs.extend_from_slice(decoder.supported_codecs());
            }

            Self::register_codecs(codecs, &mut media_engine)?;
        }

        let registry = configure_nack(Registry::new(), &mut media_engine);
        let registry = configure_rtcp_reports(registry);

        let (registry, bandwidth_estimate) = Self::init_twcc(
            registry,
            &mut media_engine,
            self.init_bandwidth,
            self.encoders.len() > 0,
            self.decoders.len() > 0,
        )?;

        let mut setting_engine = SettingEngine::default();
        setting_engine.detach_data_channels();

        // Default is too long
        setting_engine.set_ice_timeouts(None, Some(Duration::from_secs(10)), None);

        // Leave mDNS disabled on debug builds because webrtc-rs does not handle it properly when
        // communicating with another webrtc-rs instance
        #[cfg(debug_assertions)]
        setting_engine.set_ice_multicast_dns_mode(MulticastDnsMode::Disabled);

        // Enabling mDNS hides local IP addresses
        #[cfg(not(debug_assertions))]
        setting_engine.set_ice_multicast_dns_mode(MulticastDnsMode::QueryAndGather);

        let api_builder = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .build();

        let (ice_tx, ice_rx_1) = watch::channel(RTCIceConnectionState::default());
        let peer = Arc::new(WebRtcPeer {
            pc: api_builder
                .new_peer_connection(RTCConfiguration {
                    ice_servers: self.ice_servers,
                    ..Default::default()
                })
                .await?,
            signaler: Box::new(self.signaler),
            ice_tx,
            closed: Notify::new(),
        });

        // Start the WebRTC negotiation if configured to be the offerer
        match self.role {
            Role::Offerer => {
                let weak_ref = Arc::downgrade(&peer);
                peer.pc.on_negotiation_needed(Box::new(move || {
                    let peer = weak_ref.clone();
                    Box::pin(async move {
                        if let Some(peer) = peer.upgrade() {
                            if let Err(e) = peer.start_negotiation(false).await {
                                panic!("{e}");
                            }
                        }
                    })
                }));

                // Need to do this else webrtc-rs would not include audio/video in the SDP
                let codec_types: Vec<_> = self
                    .decoders
                    .iter()
                    .map(|decoder| decoder.codec_type())
                    .collect();
                for codec_type in codec_types {
                    peer.pc
                        .add_transceiver_from_kind(
                            codec_type.into(),
                            Some(RTCRtpTransceiverInit {
                                direction: RTCRtpTransceiverDirection::Recvonly,
                                send_encodings: Vec::new(),
                            }),
                        )
                        .await?;
                }
            }
            Role::Answerer => (),
        }

        // Sends the ICE candidate to the peer via the signaling channel
        let weak_ref = Arc::downgrade(&peer);
        peer.pc.on_ice_candidate(Box::new(move |candidate| {
            let peer = weak_ref.clone();
            Box::pin(async move {
                if let (Some(peer), Some(candidate)) = (peer.upgrade(), candidate) {
                    if let Ok(json) = candidate.to_json() {
                        let _ = peer.signaler.send(Message::IceCandidate(json)).await;
                    }
                }
            })
        }));

        // Monitors the ICE connection state and sends it to the encoders. Also initiates an ICE
        // restart when the connection fails.
        let weak_ref = Arc::downgrade(&peer);
        peer.pc
            .on_ice_connection_state_change(Box::new(move |state| {
                let peer = weak_ref.clone();
                Box::pin(async move {
                    if let Some(peer) = peer.upgrade() {
                        let _ = peer.ice_tx.send(state);
                        if state == RTCIceConnectionState::Failed {
                            match self.role {
                                Role::Offerer => {
                                    // TODO: Test ICE restart
                                    if let Err(e) = peer.start_negotiation(true).await {
                                        panic!("{e}");
                                    }
                                }
                                Role::Answerer => (), // Offerer should be the one to initiate ICE restart
                            }
                        }
                    }
                })
            }));

        // Close when peer connection fails
        let weak_ref = Arc::downgrade(&peer);
        peer.pc
            .on_peer_connection_state_change(Box::new(move |state| {
                let peer = weak_ref.clone();
                Box::pin(async move {
                    if state == RTCPeerConnectionState::Failed {
                        if let Some(peer) = peer.upgrade() {
                            peer.close().await;
                        }
                    }
                })
            }));

        // Spawn a task to concurrently handle the messages received from the signaling channel
        tokio::spawn(Self::signaler_message_handler(peer.clone(), self.role));

        // Handle the received track using one of the decoders
        let decoders = Arc::new(Mutex::new(self.decoders));
        let weak_ref = Arc::downgrade(&peer);
        peer.pc.on_track(Box::new(
            move |track: Arc<TrackRemote>,
                  receiver: Arc<RTCRtpReceiver>,
                  _transceiver: Arc<RTCRtpTransceiver>| {
                let decoders = decoders.clone();
                let peer = weak_ref.clone();

                // Pick one decoder that can handle the codec of the track
                Box::pin(async move {
                    if let Some(peer) = peer.upgrade() {
                        let mut decoders = decoders.lock().await;
                        let mut matched_index = None;
                        for (index, decoder) in decoders.iter().enumerate() {
                            if decoder.is_codec_supported(&track.codec().capability) {
                                matched_index = Some(index);
                                break;
                            }
                        }
                        if let Some(index) = matched_index {
                            let decoder = decoders.swap_remove(index);
                            decoder.build(track, receiver, peer);
                        }
                    }
                })
            },
        ));

        for encoder_builder in self.encoders {
            if let Some(bandwidth_estimate) = &bandwidth_estimate {
                let track = EncoderTrackLocal::new(
                    encoder_builder,
                    ice_rx_1.clone(),
                    bandwidth_estimate.clone(),
                )
                .await;
                let track = Arc::new(track);
                track.add_as_transceiver(&peer.pc).await?;
            }
        }

        if let Some(mut data_channel_handler) = self.data_channel_handler {
            match self.role {
                Role::Offerer => {
                    let data_channel = peer.pc.create_data_channel("channel", None).await?;
                    (data_channel_handler)(data_channel).await;
                }
                Role::Answerer => {
                    peer.pc.on_data_channel(data_channel_handler);
                }
            }
        }

        Ok(peer)
    }

    fn register_codecs(
        codecs: Vec<Codec>,
        media_engine: &mut MediaEngine,
    ) -> Result<(), webrtc::Error> {
        const DYNAMIC_PAYLOAD_TYPE_START: u8 = 96u8;

        let mut payload_id = Some(DYNAMIC_PAYLOAD_TYPE_START);

        for mut codec in codecs {
            if let Some(payload_type) = payload_id {
                codec.set_payload_type(payload_type);
                media_engine.register_custom_codec(codec.clone())?;
                payload_id = payload_type.checked_add(1);

                // Register for retransmission
                if let Some(mut retransmission) = Codec::retransmission(&codec) {
                    if let Some(payload_type) = payload_id {
                        retransmission.set_payload_type(payload_type);
                        media_engine.register_custom_codec(retransmission)?;
                        payload_id = payload_type.checked_add(1);
                    } else {
                        panic!("Not enough payload type for video retransmission");
                    }
                }
            } else {
                panic!("Registered too many codecs");
            }
        }

        if let Some(payload_type) = payload_id {
            // Needed for playback of non-constrained-baseline H264 for some reason
            let mut ulpfec = Codec::ulpfec();
            ulpfec.set_payload_type(payload_type);
            media_engine.register_custom_codec(ulpfec)?;
        } else {
            panic!("Not enough payload type for ULPFEC");
        }

        if let Some(payload_type) = payload_id {
            // Required for the browser to send TWCC
            let mut h264: Codec = H264Codec::constrained_baseline().into();
            h264.set_payload_type(payload_type);
            media_engine.register_custom_codec(h264)?;
        } else {
            panic!("Not enough payload type");
        }

        Ok(())
    }

    // Implements the impolite peer of "perfect negotiation".
    async fn signaler_message_handler(
        peer: Arc<WebRtcPeer>,
        role: Role,
    ) -> Result<(), webrtc::Error> {
        loop {
            if let Ok(msg) = peer.signaler.recv().await {
                match msg {
                    Message::Sdp(sdp) => {
                        let sdp_type = sdp.sdp_type;

                        if role == Role::Offerer
                            && sdp_type == RTCSdpType::Offer
                            && peer.pc.signaling_state() != RTCSignalingState::Stable
                        {
                            continue;
                        }

                        peer.pc.set_remote_description(sdp).await?;
                        if sdp_type == RTCSdpType::Offer {
                            let answer = peer.pc.create_answer(None).await?;
                            peer.pc.set_local_description(answer.clone()).await?;
                            let _ = peer.signaler.send(Message::Sdp(answer)).await;
                        }
                    }
                    Message::IceCandidate(candidate) => {
                        peer.pc.add_ice_candidate(candidate).await?;
                    }
                    Message::Bye => {
                        peer.close().await;
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn init_twcc(
        registry: Registry,
        media_engine: &mut MediaEngine,
        init_bandwidth: DataRate,
        has_encoder: bool,
        has_decoder: bool,
    ) -> Result<(Registry, Option<TwccBandwidthEstimate>), webrtc::Error> {
        match (has_encoder, has_decoder) {
            // Has a sender
            (true, _) => {
                let (registry, bandwidth_estimate) =
                    configure_custom_twcc_sender(registry, init_bandwidth)?;
                let registry = configure_twcc(registry, media_engine)?;
                Ok((registry, Some(bandwidth_estimate)))
            }
            // Only receiver
            (false, true) => {
                let registry = configure_twcc_receiver_only(registry, media_engine)?;
                Ok((registry, None))
            }
            (false, false) => Ok((registry, None)),
        }
    }
}

/// Struct representing a WebRTC connection.
///
/// Usage is through passing `EncoderBuilder`, `DecoderBuilder` and `OnDataChannelHdlrFn` to the
/// builder.
pub struct WebRtcPeer {
    pc: RTCPeerConnection,
    signaler: Box<dyn Signaler + 'static>,
    ice_tx: watch::Sender<RTCIceConnectionState>,
    closed: Notify,
}

impl WebRtcPeer {
    /// Returns a builder for a `WebRtcPeer`. The signaling channel implementation and the role
    /// of the peer needs to be both supplied.
    pub fn builder<S>(signaler: S, role: Role) -> WebRtcBuilder<S>
    where
        S: Signaler + 'static,
    {
        WebRtcBuilder::new(signaler, role)
    }

    /// Close the `WebRtcPeer`.
    pub async fn close(&self) {
        let _ = self.signaler.send(Message::Bye).await;
        let _ = self.ice_tx.send(RTCIceConnectionState::Closed);
        self.closed.notify_waiters();
    }

    /// Blocks until the `WebRtcPeer` has been closed.
    pub async fn is_closed(&self) {
        self.closed.notified().await;
    }

    async fn start_negotiation(&self, ice_restart: bool) -> Result<(), webrtc::Error> {
        let options = if ice_restart {
            Some(RTCOfferOptions {
                voice_activity_detection: false, // Seems unused
                ice_restart: true,
            })
        } else {
            None
        };

        let offer = self.pc.create_offer(options).await?;
        self.pc.set_local_description(offer.clone()).await?;
        self.signaler
            .send(Message::Sdp(offer))
            .await
            .map_err(|_| webrtc::Error::ErrUnknownType)?;
        Ok(())
    }
}

impl std::ops::Deref for WebRtcPeer {
    type Target = RTCPeerConnection;

    fn deref(&self) -> &Self::Target {
        &self.pc
    }
}

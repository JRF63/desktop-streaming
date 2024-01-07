# webrtc-helper

Wrapper for [webrtc-rs](https://github.com/webrtc-rs/webrtc) to facilitate custom encoders/decoders. It abstracts away the boilerplate-y code required to initiate a WebRTC connection through a few key traits.

Primary requirement is a signaling channel through which the SDP and ICE candidates are exchanged:

```rust
#[async_trait]
pub trait Signaler: Send + Sync {
    type Error: Send + std::fmt::Display;

    async fn recv(&self) -> Result<Message, Self::Error>;

    async fn send(&self, msg: Message) -> Result<(), Self::Error>;
}
```

`DecoderBuilder` needs to be implemented to receive encoded audio/video:

```rust
pub trait DecoderBuilder: Send {
    fn supported_codecs(&self) -> &[Codec];

    /// Data from the encoder is received through `track` while the `rtp_receiver` is used to send
    /// RTCP messages. The chosen codec is identified through `TrackRemote::codec` of `track`.
    fn build(self: Box<Self>, track: Arc<TrackRemote>, rtp_receiver: Arc<RTCRtpReceiver>);
}
```

Sending audio/video is done through `EncoderBuilder`:

```rust
pub trait EncoderBuilder: Send {
    fn supported_codecs(&self) -> &[Codec];

    /// Encoded samples are to be sent through the `RTCRtpTransceiver`. The chosen codec is found
    /// through `codec_capability`.
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
}
```

## Features

 - Includes an implementation of [Transport-Wide Congestion Control](src/interceptor/twcc/mod.rs).
 - Trickle ICE through the signaling channel
 - Can do "perfect negotation" if assumed to be the impolite peer
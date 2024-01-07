use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use webrtc::{
    ice_transport::ice_candidate::RTCIceCandidateInit,
    peer_connection::sdp::session_description::RTCSessionDescription,
};

/// The kinds of messages sent/received through the signaling channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Message {
    Sdp(RTCSessionDescription),
    IceCandidate(RTCIceCandidateInit),
    Bye,
}

/// Trait that encapsulates the WebRTC's notion of a signaling channel.
#[async_trait]
pub trait Signaler: Send + Sync {
    /// Blocks until a message is received.
    async fn recv(&self) -> Result<Message, Box<dyn std::error::Error + Send>>;

    /// Send a message through the channel.
    async fn send(&self, msg: Message) -> Result<(), Box<dyn std::error::Error + Send>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_serde() {
        let messages = [
            Message::Sdp(RTCSessionDescription::default()),
            Message::IceCandidate(RTCIceCandidateInit::default()),
            Message::Bye,
        ];
        for message in messages {
            let json = serde_json::to_string(&message).unwrap();
            println!("{json}");
            let _: Message = serde_json::from_str(&json).unwrap();
        }
    }
}

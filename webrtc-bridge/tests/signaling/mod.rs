use async_trait::async_trait;
use tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Mutex,
};
use webrtc_bridge::signaling::{Message, Signaler};

pub struct MockSignaler {
    tx: UnboundedSender<Message>,
    rx: Mutex<UnboundedReceiver<Message>>,
}

#[async_trait]
impl Signaler for MockSignaler {
    async fn recv(&self) -> Result<Message, Box<dyn std::error::Error + Send>> {
        let mut lock = self.rx.lock().await;
        let msg = lock.recv().await;
        msg.ok_or(Box::new(std::io::Error::from(
            std::io::ErrorKind::UnexpectedEof,
        )))
    }

    async fn send(&self, msg: Message) -> Result<(), Box<dyn std::error::Error + Send>> {
        match self.tx.send(msg) {
            Ok(_) => Ok(()),
            Err(_) => Err(Box::new(std::io::Error::from(
                std::io::ErrorKind::UnexpectedEof,
            ))),
        }
    }
}

impl MockSignaler {
    pub fn channel() -> (Self, Self) {
        let (tx1, rx1) = unbounded_channel();
        let (tx2, rx2) = unbounded_channel();
        let a = MockSignaler {
            tx: tx1,
            rx: Mutex::new(rx2),
        };
        let b = MockSignaler {
            tx: tx2,
            rx: Mutex::new(rx1),
        };
        (a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

    #[tokio::test]
    async fn signaler_test() {
        let (a, b) = MockSignaler::channel();

        tokio::spawn(async move {
            let msg = Message::IceCandidate(RTCIceCandidateInit {
                candidate: "test".to_owned(),
                ..RTCIceCandidateInit::default()
            });
            a.send(msg).await.unwrap();
        });

        tokio::spawn(async move {
            let msg = b.recv().await.unwrap();
            if let Message::IceCandidate(c) = msg {
                assert_eq!(c.candidate, "test");
            }
        });
    }
}

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::sync::Mutex;
use warp::ws::WebSocket;
use webrtc_helper::signaling::{Message, Signaler};

/// `Signaler` implementation using WebSocket
pub struct WebSocketSignaler {
    tx: Mutex<SplitSink<WebSocket, warp::ws::Message>>,
    rx: Mutex<SplitStream<WebSocket>>,
}

impl WebSocketSignaler {
    /// Create a new `WebSocketSignaler`.
    pub fn new(socket: WebSocket) -> WebSocketSignaler {
        let (tx, rx) = socket.split();
        WebSocketSignaler {
            tx: Mutex::new(tx),
            rx: Mutex::new(rx),
        }
    }

    async fn recv_impl(&self) -> Result<Message, WebSocketSignalerError> {
        match self.rx.lock().await.next().await {
            Some(ws_msg) => match ws_msg?.to_str() {
                Ok(s) => {
                    let msg = serde_json::from_str::<Message>(s)?;
                    Ok(msg)
                }
                _ => Err(WebSocketSignalerError::Serde),
            },
            _ => Err(WebSocketSignalerError::Eof), // closed
        }
    }

    async fn send_impl(&self, msg: Message) -> Result<(), WebSocketSignalerError> {
        let s = serde_json::to_string(&msg)?;
        let ws_msg = warp::ws::Message::text(s);
        self.tx.lock().await.send(ws_msg).await?;
        Ok(())
    }
}

/// Errors that WebSocketSignaler can emit
#[derive(Debug)]
pub enum WebSocketSignalerError {
    Warp,
    Serde,
    Eof,
}

impl std::fmt::Display for WebSocketSignalerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSocketSignalerError::Warp => {
                write!(f, "Encountered an error in the underlying WebSocket")
            }
            WebSocketSignalerError::Serde => write!(f, "Failed to deserialize the message"),
            WebSocketSignalerError::Eof => {
                write!(f, "WebSocket connection has been closed")
            }
        }
    }
}

impl std::error::Error for WebSocketSignalerError {}

// The conversion only cares about the error type and discards the error details.
macro_rules! impl_from {
    ($t:ty, $e:tt) => {
        impl From<$t> for WebSocketSignalerError {
            #[inline]
            fn from(_: $t) -> Self {
                WebSocketSignalerError::$e
            }
        }
    };
}

impl_from!(warp::Error, Warp);
impl_from!(serde_json::Error, Serde);

#[async_trait::async_trait]
impl Signaler for WebSocketSignaler {
    async fn recv(&self) -> Result<Message, Box<dyn std::error::Error + Send>> {
        match self.recv_impl().await {
            Ok(msg) => Ok(msg),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn send(&self, msg: Message) -> Result<(), Box<dyn std::error::Error + Send>> {
        match self.send_impl(msg).await {
            Ok(()) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

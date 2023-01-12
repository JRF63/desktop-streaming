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
}

/// Errors that WebSocketSignaler can emit
pub enum WebSocketSignalerError {
    Warp(warp::Error),
    Serde(serde_json::Error),
    MessageToStr,
    Eof,
}

impl std::fmt::Display for WebSocketSignalerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSocketSignalerError::Warp(e) => e.fmt(f),
            WebSocketSignalerError::Serde(e) => e.fmt(f),
            WebSocketSignalerError::MessageToStr => {
                write!(f, "Failed to convert WebSocket message to a `&str`")
            }
            WebSocketSignalerError::Eof => {
                write!(f, "WebSocket connection has been closed")
            }
        }
    }
}

impl From<warp::Error> for WebSocketSignalerError {
    fn from(value: warp::Error) -> Self {
        WebSocketSignalerError::Warp(value)
    }
}

impl From<serde_json::Error> for WebSocketSignalerError {
    fn from(value: serde_json::Error) -> Self {
        WebSocketSignalerError::Serde(value)
    }
}

#[async_trait::async_trait]
impl Signaler for WebSocketSignaler {
    type Error = WebSocketSignalerError;

    async fn recv(&self) -> Result<Message, Self::Error> {
        match self.rx.lock().await.next().await {
            Some(ws_msg) => match ws_msg?.to_str() {
                Ok(s) => {
                    let msg = serde_json::from_str::<Message>(s)?;
                    Ok(msg)
                }
                _ => Err(WebSocketSignalerError::MessageToStr),
            },
            _ => Err(WebSocketSignalerError::Eof), // closed
        }
    }

    async fn send(&self, msg: Message) -> Result<(), Self::Error> {
        let s = serde_json::to_string(&msg)?;
        let ws_msg = warp::ws::Message::text(s);
        self.tx.lock().await.send(ws_msg).await?;
        Ok(())
    }
}

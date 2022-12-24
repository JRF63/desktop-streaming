use std::net::SocketAddr;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::sync::Mutex;
use warp::{
    http::{Response, StatusCode},
    ws::WebSocket,
    Filter,
};
use webrtc_helper::signaling::{Message, Signaler};

const INDEX: &'static str = include_str!("html/index.html");
const NOT_FOUND: &'static str = include_str!("html/not_found.html");

pub struct WebSocketSignaler {
    tx: Mutex<SplitSink<WebSocket, warp::ws::Message>>,
    rx: Mutex<SplitStream<WebSocket>>,
}

impl WebSocketSignaler {
    fn new(
        tx: SplitSink<WebSocket, warp::ws::Message>,
        rx: SplitStream<WebSocket>,
    ) -> WebSocketSignaler {
        WebSocketSignaler {
            tx: Mutex::new(tx),
            rx: Mutex::new(rx),
        }
    }
}

#[async_trait::async_trait]
impl Signaler for WebSocketSignaler {
    async fn recv(&self) -> std::io::Result<Message> {
        match self.rx.lock().await.next().await {
            Some(Ok(ws_msg)) => {
                if let Ok(s) = ws_msg.to_str() {
                    match serde_json::from_str::<Message>(s) {
                        Ok(msg) => Ok(msg),
                        Err(_) => Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)),
                    }
                } else {
                    Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
                }
            }
            _ => Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)), // closed
        }
    }

    async fn send(&self, msg: Message) -> std::io::Result<()> {
        if let Ok(s) = serde_json::to_string(&msg) {
            let ws_msg = warp::ws::Message::text(s);
            if let Err(_) = self.tx.lock().await.send(ws_msg).await {
                return Ok(());
            }
        }
        Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
    }
}

pub async fn http_server(addr: impl Into<SocketAddr>) {
    // GET /
    let index = warp::path::end().map(|| Response::new(INDEX));

    // 404
    let not_found = warp::path::peek().map(|_| {
        let mut response = Response::new(NOT_FOUND);
        *response.status_mut() = StatusCode::NOT_FOUND;
        response
    });

    let websocket = warp::path("ws")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(move |socket| process_websocket(socket)));

    let routes = index.or(websocket).or(not_found);

    warp::serve(routes).run(addr).await;
}

async fn process_websocket(socket: WebSocket) {
    let (ws_tx, ws_rx) = socket.split();
    let websocket_signaler = WebSocketSignaler::new(ws_tx, ws_rx);
}

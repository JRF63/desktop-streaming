use crate::encoder::NvidiaEncoderBuilder;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::sync::Mutex;
use warp::{
    http::{Response, StatusCode},
    ws::WebSocket,
    Filter,
};
use webrtc_helper::{
    peer::Role,
    signaling::{Message, Signaler},
    WebRtcBuilder,
};

const INDEX: &'static str = include_str!("html/index.html");
const NOT_FOUND: &'static str = include_str!("html/not_found.html");

pub struct WebSocketSignaler {
    socket: Mutex<WebSocket>,
}

impl WebSocketSignaler {
    fn new(socket: WebSocket) -> WebSocketSignaler {
        WebSocketSignaler {
            socket: Mutex::new(socket),
        }
    }
}

#[async_trait::async_trait]
impl Signaler for WebSocketSignaler {
    async fn recv(&self) -> std::io::Result<Message> {
        match self.socket.lock().await.next().await {
            Some(Ok(ws_msg)) => match ws_msg.to_str() {
                Ok(s) => match serde_json::from_str::<Message>(s) {
                    Ok(msg) => Ok(msg),
                    Err(_) => Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)),
                },
                Err(_) => Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)),
            },
            _ => Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)), // closed
        }
    }

    async fn send(&self, msg: Message) -> std::io::Result<()> {
        if let Ok(s) = serde_json::to_string(&msg) {
            let ws_msg = warp::ws::Message::text(s);
            if let Err(_) = self.socket.lock().await.send(ws_msg).await {
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
    let websocket_signaler = WebSocketSignaler::new(socket);

    // TODO: Debug

    tokio::spawn(async move {
        let mut encoder_builder = WebRtcBuilder::new(websocket_signaler, Role::Offerer);
        encoder_builder.with_encoder(Box::new(NvidiaEncoderBuilder::new()));
        let encoder = encoder_builder.build().await.unwrap();
        while !encoder.is_closed() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
}

use crate::nvidia::NvidiaEncoderBuilder;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
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

#[cfg(not(debug_assertions))]
const INDEX: &'static str = include_str!("html/index.html");
const NOT_FOUND: &'static str = include_str!("html/not_found.html");

pub struct WebSocketSignaler {
    tx: Mutex<SplitSink<WebSocket, warp::ws::Message>>,
    rx: Mutex<SplitStream<WebSocket>>,
}

impl WebSocketSignaler {
    fn new(socket: WebSocket) -> WebSocketSignaler {
        let (tx, rx) = socket.split();
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
            if let Ok(_) = self.tx.lock().await.send(ws_msg).await {
                return Ok(());
            }
        }
        Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
    }
}

pub async fn http_server(addr: impl Into<SocketAddr>) {
    // GET /
    let index = warp::path::end().map(|| {
        #[cfg(not(debug_assertions))]
        {
            Response::new(INDEX)
        }

        #[cfg(debug_assertions)]
        {
            use std::fs::File;
            use std::io::prelude::*;

            let mut file = File::open("src/html/index.html").unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();
            Response::new(contents)
        }
    });

    // 404
    let not_found = warp::path::peek().map(|_| {
        let mut response = Response::new(NOT_FOUND);
        *response.status_mut() = StatusCode::NOT_FOUND;
        response
    });

    let websocket = warp::path("ws")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(process_websocket));

    let routes = index.or(websocket).or(not_found);

    warp::serve(routes).run(addr).await;
}

async fn process_websocket(socket: WebSocket) {
    let websocket_signaler = WebSocketSignaler::new(socket);

    // TODO: Debug

    log::info!("WebSocket upgrade");

    tokio::spawn(async move {
        let mut encoder_builder = WebRtcBuilder::new(websocket_signaler, Role::Offerer);
        encoder_builder.with_encoder(Box::new(NvidiaEncoderBuilder::new("display-mirror".to_owned(), "0".to_owned())));
        let encoder = encoder_builder.build().await.unwrap();
        while !encoder.is_closed() {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
}

use crate::{input::controls_handler, nvidia::NvidiaEncoderBuilder, signaler::WebSocketSignaler};
use std::{
    net::SocketAddr,
    sync::atomic::{AtomicBool, Ordering},
};
use warp::{
    http::{Response, StatusCode},
    ws::WebSocket,
    Filter,
};
use webrtc_helper::{peer::Role, WebRtcBuilder};

#[cfg(not(debug_assertions))]
const INDEX: &'static str = include_str!("html/index.html");
const NOT_FOUND: &'static str = include_str!("html/not_found.html");

static DUPLICATOR_RUNNING: AtomicBool = AtomicBool::new(false);

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

            let mut file = File::open("server-windows/src/html/index.html").unwrap();
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

    let websocket = warp::path::end()
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(process_websocket));

    let routes = websocket.or(index).or(not_found);

    warp::serve(routes).run(addr).await;
}

async fn process_websocket(socket: WebSocket) {
    if DUPLICATOR_RUNNING.load(Ordering::Acquire) {
        return;
    }

    DUPLICATOR_RUNNING.store(true, Ordering::Release);

    let websocket_signaler = WebSocketSignaler::new(socket);

    log::info!("WebSocket upgrade");

    tokio::spawn(async move {
        let mut encoder_builder = WebRtcBuilder::new(websocket_signaler, Role::Answerer);
        encoder_builder
            .with_encoder(Box::new(NvidiaEncoderBuilder::new(
                "display-mirror".to_owned(),
                "0".to_owned(),
            )))
            .with_data_channel_handler(Box::new(controls_handler));
        let encoder = encoder_builder.build().await.unwrap();
        encoder.is_closed().await;
        DUPLICATOR_RUNNING.store(false, Ordering::Release);
        log::info!("Exited");
    });
}

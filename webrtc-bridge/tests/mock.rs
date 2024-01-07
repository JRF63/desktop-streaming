mod codec;
mod decoder;
mod encoder;
mod signaling;

use self::{decoder::MockDecoderBuilder, encoder::MockEncoderBuilder, signaling::MockSignaler};
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;
use webrtc_bridge::peer::{Role, WebRtcBuilder};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_test() {
    let (encoder_signaler, decoder_signaler) = MockSignaler::channel();

    let stop_1 = Arc::new(Notify::new());
    let stop_2 = stop_1.clone();
    let stop_3 = stop_1.clone();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        stop_1.notify_waiters();
    });

    let handle_1 = tokio::spawn(async move {
        let mut encoder_builder = WebRtcBuilder::new(encoder_signaler, Role::Offerer);
        encoder_builder.with_encoder(Box::new(MockEncoderBuilder::new()));
        let encoder = encoder_builder.build().await.unwrap();
        stop_2.notified().await;
        encoder.close().await;
    });

    let handle_2 = tokio::spawn(async move {
        let mut decoder_builder = WebRtcBuilder::new(decoder_signaler, Role::Answerer);
        decoder_builder.with_decoder(Box::new(MockDecoderBuilder::new()));
        let decoder = decoder_builder.build().await.unwrap();
        stop_3.notified().await;
        decoder.close().await;
    });

    let _ = handle_1.await;
    let _ = handle_2.await;

    tokio::time::sleep(Duration::from_secs(1)).await;
}

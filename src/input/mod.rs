mod pointer;

use self::pointer::{PointerDevice, PointerEvent};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::Mutex, time::interval};
use webrtc::{data::data_channel::DataChannel, data_channel::RTCDataChannel};
use windows::Win32::UI::Controls::POINTER_TYPE_INFO;

const MESSAGE_SIZE: usize = 1500;

pub fn controls_handler(
    data_channel: Arc<RTCDataChannel>,
) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
    Box::pin(async move {
        let data_channel = Arc::clone(&data_channel);
        let data_channel_2 = Arc::clone(&data_channel);
        data_channel_2.on_open(Box::new(move || {
            Box::pin(async move {
                let raw = match data_channel.detach().await {
                    Ok(raw) => raw,
                    Err(err) => {
                        log::error!("data channel detach got err: {}", err);
                        return;
                    }
                };

                let raw = Arc::clone(&raw);
                tokio::spawn(async move {
                    let _ = control_loop(raw).await;
                });
            })
        }));
    })
}

async fn control_loop(data_channel: Arc<DataChannel>) {
    let sender = Arc::new(Mutex::new(Vec::<POINTER_TYPE_INFO>::new()));
    let receiver = sender.clone();
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stopper = stop_signal.clone();

    tokio::spawn(async move {
        let mut buffer = vec![0u8; MESSAGE_SIZE];

        while let Ok((n, is_string)) = data_channel.read_data_channel(&mut buffer).await {
            if is_string {
                if let Ok(s) = std::str::from_utf8(&buffer[..n]) {
                    match serde_json::from_str::<PointerEvent>(s) {
                        Ok(p) => {
                            let mut sender = sender.lock().await;
                            sender.push(p.into());
                        }
                        Err(e) => log::error!("serde_json::from_str error: {e}"),
                    }
                }
            }
        }
        stopper.store(true, Ordering::Release);
    });

    tokio::spawn(async move {
        let device = PointerDevice::new().expect("Failed to create `PointerDevice`");
        let mut interval = interval(Duration::from_micros(100));

        while !stop_signal.load(Ordering::Acquire) {
            interval.tick().await;
            let mut receiver = receiver.lock().await;
            if let Err(e) = device.inject_pointer_input(receiver.as_slice()) {
                log::error!("inject_pointer_input error: {e}");
            }
            receiver.clear();
        }
    });
}

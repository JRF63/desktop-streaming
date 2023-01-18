mod pointer;

use self::pointer::{PointerDevice, PointerEvent};
use std::{future::Future, pin::Pin, sync::Arc};
use webrtc::{data::data_channel::DataChannel, data_channel::RTCDataChannel};
use windows::{
    core::HRESULT,
    Win32::{Foundation::ERROR_NOT_READY, UI::Controls::POINTER_TYPE_INFO},
};

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
    let device = PointerDevice::new().expect("Failed to create `PointerDevice`");
    let mut buffer = vec![0u8; MESSAGE_SIZE];

    let not_ready = HRESULT(ERROR_NOT_READY.0 as _);

    while let Ok((n, is_string)) = data_channel.read_data_channel(&mut buffer).await {
        if !is_string {
            continue;
        }

        if let Ok(s) = std::str::from_utf8(&buffer[..n]) {
            match serde_json::from_str::<PointerEvent>(s) {
                Ok(p) => {
                    let p: POINTER_TYPE_INFO = p.into();

                    loop {
                        match device.inject_pointer_input(std::array::from_ref(&p)) {
                            Ok(_) => break,
                            Err(e) => {
                                if e.code() == not_ready {
                                    continue;
                                }
                                log::error!("inject_pointer_input error: {e}");
                                break;
                            }
                        }
                    }
                }
                Err(e) => log::error!("serde_json::from_str error: {e}"),
            }
        }
    }
}

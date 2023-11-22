# WebRTC Desktop Streamer

A Rust lang streaming app in the same vein as Steam Link, [Parsec](https://parsec.app/) and [Moonlight](https://github.com/moonlight-stream). And has touch/pen passthrough support unlike those three. 

## Usage

```sh
cargo run --release
```

then use the browser to go to the PC's IP address at port 9090.

## Performance

![latency](.github/latency.png?raw=true)

Achievable latency is around 50 ms. The breakdown being:
- ~16 ms inside the encoder
- \< 1 ms through the network
- leaving about 33 ms for the browser's decoding?

## How it works

The video output is done by capturing the desktop though Windows' [IDXGIOutputDuplication](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgioutputduplication) API, encodes it using NvEnc, fragments the resulting NAL's, then shoves it through [webrtc-rs](https://github.com/webrtc-rs/webrtc).

Touch/pen input is simulated through the [InjectSyntheticPointerInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-injectsyntheticpointerinput) API with the data coming from the browser's [PointerEvent](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent).

WebRTC signaling is handled through WebSocket's. The plan being to support both browser and native client using the same server implementation.

## TODO
- [ ] Make it work on Firefox
- [ ] Native client for lower latency and to have support for HEVC
- [x] Handle reconnect after disconnection without closing the server
- [ ] Encrypt WebSocket comm. with TLS using a self-signed cert
- [ ] Some type of authentication (JWT maybe?)
- [ ] AMD, Intel encoders
- [ ] Linux server using PipeWire
- [ ] Gamepad support
- [ ] Audio streaming via Opus
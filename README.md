# WebRTC Desktop Streamer

![latency](https://github.com/JRF63/desktop-streaming/raw/dev/.github/latency.png)

Achievable latency is around 50 ms. The breakdown being:
- ~16 ms inside the encoder
- \< 1 ms through the network
- leaving 33 ms for the browser's decoding

## TODO
- Make it work on Firefox
- Native client for lower latency and to have support for HEVC
- Handle reconnect after disconnection without closing the server
- AMD, Intel encoders
- Linux server using PipeWire
- Gamepad support
- Audio streaming via Opus
use audio_codec::{
    decoder::AudioDecoder,
    encoder::AudioEncoder,
    settings::{ApplicationMode, AudioChannels, Bitrate, SampleRate},
};
use rodio::{source::SineWave, OutputStream};

#[test]
fn encode_decode() {
    const SAMPLE_RATE: SampleRate = SampleRate::Fullband;
    const NUM_CHANNELS: AudioChannels = AudioChannels::Mono;
    const SINE_FREQ: f32 = 600.0;
    const NUM_SAMPLES: i32 = 480;
    const N: usize = 10;

    let mut data: Vec<Vec<u8>> = Vec::new();
    let mut decoded: Vec<f32> = Vec::new();

    let mut sine_wave = SineWave::new(SINE_FREQ);

    let mut encoder =
        AudioEncoder::new(SAMPLE_RATE, NUM_CHANNELS, ApplicationMode::LowDelay).unwrap();
    let mut decoder = AudioDecoder::new(SAMPLE_RATE, NUM_CHANNELS).unwrap();

    encoder.set_bitrate(Bitrate::new(128000).unwrap()).unwrap();

    for _ in 0..N {
        let samples: Vec<_> = sine_wave.by_ref().take(NUM_SAMPLES as usize).collect();
        let mut output = vec![0; 128000];
        let bytes_encoded = encoder.encode(&samples, &mut output).unwrap();
        output.truncate(bytes_encoded as usize);
        data.push(output);
    }

    for encoded in data {
        let mut output = vec![0f32; 128000];
        let samples_decoded = decoder.decode(&encoded, &mut output, false).unwrap();
        output.truncate(samples_decoded as usize);


    }

    // let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    // stream_handle.play_raw(source).unwrap();
}

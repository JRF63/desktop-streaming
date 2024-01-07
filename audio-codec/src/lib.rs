pub mod decoder;
pub mod encoder;
pub mod error;
pub mod settings;
pub mod sys;

mod codec_traits;

#[cfg(test)]
mod test {
    use super::{
        decoder::AudioDecoder,
        encoder::AudioEncoder,
        settings::{ApplicationMode, AudioChannels, Bitrate, SampleRate},
    };
    use rand::{
        distributions::{Distribution, Uniform},
        rngs::StdRng,
        SeedableRng,
    };

    #[test]
    fn audio_codec_encode_decode_white_noise() {
        fn build_white_noise_samples(num_frames: usize, phase_shift: usize) -> Vec<f32> {
            let mut rng = StdRng::seed_from_u64(0);
            // Opus will just clip outside this range
            let uniform_dist = Uniform::new_inclusive(-1.0f32, 1.0f32);

            // White noise
            let monoaural_samples: Vec<f32> = (0..(num_frames + 2 * phase_shift))
                .map(|_| uniform_dist.sample(&mut rng))
                .collect();

            fn map_range(value: usize, num_frames: usize, phase_shift: usize) -> isize {
                let scaled = value as f64 / num_frames as f64;
                let min_shift = -(phase_shift as isize);
                min_shift + (scaled * (2 * phase_shift + 1) as f64) as isize
            }

            // Phase shift the white noise to simulate directional audio
            (0..num_frames)
                .map(|i| {
                    let left = i + phase_shift;
                    let shift = map_range(i, num_frames, phase_shift);
                    let right = left as isize + shift;
                    [monoaural_samples[left], monoaural_samples[right as usize]]
                })
                .flatten()
                .collect()
        }

        const SAMPLE_RATE: SampleRate = SampleRate::Fullband;
        const NUM_CHANNELS: AudioChannels = AudioChannels::Stereo;

        const PHASE_SHIFT: usize = 32;

        const NUM_FRAMES_IN_WINDOW: usize = 480;
        const NUM_WINDOWS: usize = 400;

        let binaural_samples =
            build_white_noise_samples(NUM_WINDOWS * NUM_FRAMES_IN_WINDOW, PHASE_SHIFT);

        let mut encoder =
            AudioEncoder::new(SAMPLE_RATE, NUM_CHANNELS, ApplicationMode::LowDelay).unwrap();
        let mut decoder = AudioDecoder::new(SAMPLE_RATE, NUM_CHANNELS).unwrap();

        encoder.set_bitrate(Bitrate::new(128000).unwrap()).unwrap();

        // Size is at most as big as the number of bytes in the window else it would not be much of a
        // lossy encoder
        let encoded_buf_len = std::mem::size_of::<f32>() * NUM_FRAMES_IN_WINDOW;

        let mut encoded = vec![0; encoded_buf_len];
        let mut decoded = vec![0f32; binaural_samples.len()];
        let mut decoded_idx = 0;

        for samples in binaural_samples.chunks_exact(NUM_CHANNELS as usize * NUM_FRAMES_IN_WINDOW) {
            encoded.resize_with(encoded_buf_len, Default::default);
            let num_bytes_encoded = encoder.encode(samples, &mut encoded).unwrap();
            encoded.truncate(num_bytes_encoded as usize);

            let num_samples_decoded = NUM_CHANNELS as i32
                * decoder
                    .decode(&encoded, &mut decoded[decoded_idx..], false)
                    .unwrap();
            decoded_idx += num_samples_decoded as usize;
        }

        #[cfg(feature = "has_audio_output_device")]
        {
            use rodio::{buffer::SamplesBuffer, OutputStream};

            // This should play white noise that appears to move from left to right
            let buffer = SamplesBuffer::new(NUM_CHANNELS as _, SAMPLE_RATE as _, decoded);
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            stream_handle.play_raw(buffer).unwrap();

            let secs = (NUM_WINDOWS * NUM_FRAMES_IN_WINDOW) as f64 / SAMPLE_RATE as i32 as f64;
            let millis = (secs * 1000.0 + 250.0) as u64;
            std::thread::sleep(std::time::Duration::from_millis(millis));
        }
    }
}

use super::encoder::start_encoder;
use crate::{capture::ScreenDuplicator, device::create_d3d11_device};
use std::{collections::HashMap, sync::Arc};
use webrtc::{
    rtp_transceiver::{rtp_codec::RTCRtpCodecCapability, RTCRtpTransceiver},
    track::track_local::track_local_static_rtp::TrackLocalStaticRTP,
};
use webrtc_helper::{
    codecs::{Codec, CodecType, H264Codec, H264Profile},
    encoder::EncoderBuilder,
    interceptor::twcc::TwccBandwidthEstimate,
    peer::IceConnectionState,
};
use windows::Win32::Graphics::{
    Direct3D11::ID3D11Device,
    Dxgi::Common::{
        DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM,
        DXGI_FORMAT_R8G8B8A8_UNORM,
    },
};

pub struct NvidiaEncoderBuilder {
    inner_builder: nvenc::EncoderBuilder<nvenc::DirectX11Device>,
    device: ID3D11Device,
    id: String,
    stream_id: String,
    display_index: u32,
    display_formats: Vec<DXGI_FORMAT>,
    supported_codecs: Vec<Codec>,
}

impl EncoderBuilder for NvidiaEncoderBuilder {
    fn id(&self) -> &str {
        &self.id
    }

    fn stream_id(&self) -> &str {
        &self.stream_id
    }

    fn codec_type(&self) -> CodecType {
        CodecType::Video
    }

    fn supported_codecs(&self) -> &[Codec] {
        &self.supported_codecs
    }

    fn build(
        mut self: Box<Self>,
        rtp_track: Arc<TrackLocalStaticRTP>,
        transceiver: Arc<RTCRtpTransceiver>,
        ice_connection_state: IceConnectionState,
        bandwidth_estimate: TwccBandwidthEstimate,
        codec_capability: RTCRtpCodecCapability,
        ssrc: u32,
        payload_type: u8,
    ) {
        if !self.is_codec_supported(&codec_capability) {
            panic!("Codec not supported");
        }

        let screen_duplicator =
            match ScreenDuplicator::new(self.device, self.display_index, self.display_formats) {
                Ok(duplicator) => duplicator,
                Err(e) => {
                    panic!("Failed to create `ScreenDuplicator`: {e}");
                }
            };

        let (codec, profile) = {
            match codec_capability.mime_type.as_str() {
                "video/H264" => {
                    let profile =
                        match h264_profile_from_sdp_fmtp_line(&codec_capability.sdp_fmtp_line) {
                            Some(profile) => profile,
                            None => panic!(
                                "Unable to parse {} as H.264 profile",
                                codec_capability.sdp_fmtp_line
                            ),
                        };
                    (nvenc::Codec::H264, profile)
                }
                "video/H265" => {
                    todo!("Implement HEVC parsing")
                }
                "video/AV1" => todo!("AV1 is not supported by the nvenc version used"),
                _ => panic!("Unsupported codec"),
            }
        };

        log::info!("NvidiaEncoderBuilder::build with codec {codec:?} and profile {profile:?}");

        if let Err(e) = self.inner_builder.with_codec(codec) {
            panic!("Encoder does not support the codec `{codec:?}`: {e}");
        }

        let supported_encode_presets = match self.inner_builder.supported_encode_presets(codec) {
            Ok(supported_encode_presets) => supported_encode_presets,
            Err(e) => {
                panic!("Failed to query encode presets: {e}");
            }
        };

        let new_settings = supported_encode_presets.contains(&nvenc::EncodePreset::P4);

        let (preset, tuning_info, multi_pass, _rc_mode) = if new_settings {
            // Equivalent settings for the old LowLatencyDefault and CBR_HQ:
            // https://docs.nvidia.com/video-technologies/video-codec-sdk/nvenc-preset-migration-guide/
            (
                nvenc::EncodePreset::P4,
                nvenc::TuningInfo::UltraLowLatency,
                nvenc::MultiPassSetting::FullResolution,
                Option::<()>::None, // TODO: Implement RC mode settings
            )
        } else {
            todo!("Need to first implement RC mode settings in nvenc")
        };

        let configure_encoder =
            |builder: &mut nvenc::EncoderBuilder<nvenc::DirectX11Device>| -> nvenc::Result<()> {
                builder
                    .with_codec_profile(profile)?
                    .with_encode_preset(preset)?
                    .with_tuning_info(tuning_info)?
                    .set_multi_pass(multi_pass)?;
                // TODO: set_rc_mode(rc_mode)
                Ok(())
            };

        if let Err(e) = configure_encoder(&mut self.inner_builder) {
            panic!("Error configuring encoder: {e}");
        }

        let (width, height, texture_format) = {
            let display_desc = screen_duplicator.desc();
            let mode_desc = &display_desc.ModeDesc;
            (mode_desc.Width, mode_desc.Height, mode_desc.Format)
        };

        let (input, output) = match self.inner_builder.build(width, height, texture_format) {
            Ok((input, output)) => (input, output),
            Err(e) => {
                panic!("Failed to build encoder: {e}");
            }
        };

        let handle = tokio::runtime::Handle::current();
        handle.spawn(start_encoder(
            screen_duplicator,
            input,
            output,
            rtp_track,
            transceiver,
            ice_connection_state,
            bandwidth_estimate,
            payload_type,
            ssrc,
            codec_capability.clock_rate,
        ));
    }
}

impl NvidiaEncoderBuilder {
    pub fn new(id: String, stream_id: String) -> NvidiaEncoderBuilder {
        log::info!("NvidiaEncoderBuilder::new");
        let device = match create_d3d11_device() {
            Ok(device) => device,
            Err(e) => {
                panic!("Unable to create D3D11Device: {e}");
            }
        };
        let mut inner_builder = match nvenc::EncoderBuilder::new(device.clone()) {
            Ok(inner_builder) => inner_builder,
            Err(e) => {
                log::error!("{e}");
                panic!("Error while creating the encoder: {e}");
            }
        };
        if let Err(e) = inner_builder.repeat_csd(true) {
            panic!("Error while setting encoder option: {e}");
        }

        let display_index = 0; // default to the first; could be changed later
        let display_formats = vec![
            DXGI_FORMAT_B8G8R8A8_UNORM,
            DXGI_FORMAT_R10G10B10A2_UNORM,
            DXGI_FORMAT_R8G8B8A8_UNORM,
        ];
        let supported_codecs = match list_supported_codecs(&mut inner_builder) {
            Ok(supported_codecs) => supported_codecs,
            Err(e) => {
                panic!("Unable to list codecs: {e}");
            }
        };

        NvidiaEncoderBuilder {
            inner_builder,
            device,
            id,
            stream_id,
            display_index,
            display_formats,
            supported_codecs,
        }
    }

    #[allow(dead_code)]
    pub fn set_display_index(&mut self, display_index: u32) {
        self.display_index = display_index;
    }
}

fn list_supported_codecs(
    inner_builder: &mut nvenc::EncoderBuilder<nvenc::DirectX11Device>,
) -> nvenc::Result<Vec<Codec>> {
    let mut codecs: Vec<Codec> = Vec::new();
    for codec in inner_builder.supported_codecs()? {
        match codec {
            nvenc::Codec::H264 => {
                let sorter = HashMap::from([
                    (nvenc::CodecProfile::H264ConstrainedHigh, 0),
                    (nvenc::CodecProfile::H264High, 1),
                    (nvenc::CodecProfile::H264Main, 2),
                    (nvenc::CodecProfile::H264Baseline, 3),
                ]);

                let mut supported_codec_profiles = inner_builder.supported_codec_profiles(codec)?;
                supported_codec_profiles.sort_by(|a, b| match (sorter.get(a), sorter.get(b)) {
                    (Some(x), Some(y)) => x.cmp(y),
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                });

                let convert_h264_profile = |profile: nvenc::CodecProfile| -> Option<H264Profile> {
                    match profile {
                        nvenc::CodecProfile::H264Baseline => Some(H264Profile::Baseline),
                        nvenc::CodecProfile::H264Main => Some(H264Profile::Main),
                        nvenc::CodecProfile::H264High => Some(H264Profile::High),
                        nvenc::CodecProfile::H264High444 => Some(H264Profile::High444),
                        nvenc::CodecProfile::H264Stereo => Some(H264Profile::StereoHigh),
                        nvenc::CodecProfile::H264ProgressiveHigh => {
                            Some(H264Profile::ProgressiveHigh)
                        }
                        nvenc::CodecProfile::H264ConstrainedHigh => {
                            Some(H264Profile::ConstrainedHigh)
                        }
                        nvenc::CodecProfile::HevcMain
                        | nvenc::CodecProfile::HevcMain10
                        | nvenc::CodecProfile::HevcFrext => {
                            panic!("Unexpected HEVC profile returned while using H264 codec");
                        }
                        nvenc::CodecProfile::Autoselect => None, // Always present
                        _ => None,                               // Unknown profile
                    }
                };

                for profile in supported_codec_profiles {
                    if let Some(profile) = convert_h264_profile(profile) {
                        codecs.push(H264Codec::new(profile).into());
                    }
                }
            }
            nvenc::Codec::Hevc => {
                // TODO: Not yet supported
                continue;
            }
            _ => {
                // TODO: Possibly AV1
                continue;
            }
        }
    }
    Ok(codecs)
}

fn h264_profile_from_sdp_fmtp_line(sdp_fmtp_line: &str) -> Option<nvenc::CodecProfile> {
    if let Some((_, id)) = sdp_fmtp_line.split_once("profile-level-id=") {
        if id.len() >= 6 {
            if let Ok(profile) = H264Profile::from_str(id) {
                match profile {
                    H264Profile::ConstrainedBaseline | H264Profile::Baseline => {
                        return Some(nvenc::CodecProfile::H264Baseline)
                    }
                    H264Profile::Main | H264Profile::Extended => {
                        return Some(nvenc::CodecProfile::H264Main)
                    }
                    H264Profile::High | H264Profile::High10 | H264Profile::High422 => {
                        return Some(nvenc::CodecProfile::H264High)
                    }
                    H264Profile::ProgressiveHigh => {
                        return Some(nvenc::CodecProfile::H264ProgressiveHigh)
                    }
                    H264Profile::ConstrainedHigh => {
                        return Some(nvenc::CodecProfile::H264ConstrainedHigh)
                    }
                    H264Profile::High444 => return Some(nvenc::CodecProfile::H264High444),
                    H264Profile::StereoHigh => return Some(nvenc::CodecProfile::H264Stereo),
                    H264Profile::High10Intra
                    | H264Profile::High422Intra
                    | H264Profile::High444Intra
                    | H264Profile::Cavlc444Intra => (),
                    _ => (),
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h264_ftmp_line_parsing() {
        let test_cases = [
            "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42001f",
            "level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42001f",
            "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f",
            "level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42e01f",
            "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=4d001f",
            "level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=4d001f",
            "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=64001f",
            // reordered
            "level-asymmetry-allowed=1;profile-level-id=42001f;packetization-mode=1",
            "profile-level-id=42001f;level-asymmetry-allowed=1;packetization-mode=1",
            // extra
            "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=640c1f",
        ];

        let profiles = [
            nvenc::CodecProfile::H264Baseline,
            nvenc::CodecProfile::H264Baseline,
            nvenc::CodecProfile::H264Baseline,
            nvenc::CodecProfile::H264Baseline,
            nvenc::CodecProfile::H264Main,
            nvenc::CodecProfile::H264Main,
            nvenc::CodecProfile::H264High,
            nvenc::CodecProfile::H264Baseline,
            nvenc::CodecProfile::H264Baseline,
            nvenc::CodecProfile::H264ConstrainedHigh,
        ];

        for (sdp_fmtp_line, profile) in test_cases.iter().zip(profiles) {
            assert_eq!(
                h264_profile_from_sdp_fmtp_line(sdp_fmtp_line),
                Some(profile)
            );
        }
    }
}

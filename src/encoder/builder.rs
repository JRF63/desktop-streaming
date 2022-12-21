use crate::{capture::ScreenDuplicator, device::create_d3d11_device};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};
use webrtc::{
    rtp_transceiver::rtp_codec::RTCRtpCodecParameters, track::track_local::TrackLocalContext,
};
use webrtc_helper::{
    codecs::Codec,
    encoder::{Encoder, EncoderBuilder},
};
use windows::Win32::Graphics::{
    Direct3D11::ID3D11Device,
    Dxgi::Common::{
        DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM,
        DXGI_FORMAT_R8G8B8A8_UNORM,
    },
};

static INCREMENTAL_ID: AtomicUsize = AtomicUsize::new(0);

pub struct NvidiaEncoderBuilder {
    inner_builder: nvenc::EncoderBuilder<nvenc::DirectX11Device>,
    device: ID3D11Device,
    id: String,
    display_index: u32,
    display_formats: Vec<DXGI_FORMAT>,
    supported_codecs: Vec<Codec>,
}

impl EncoderBuilder for NvidiaEncoderBuilder {
    fn id(&self) -> &str {
        &self.id
    }

    fn stream_id(&self) -> &str {
        "screen-duplicator"
    }

    fn supported_codecs(&self) -> &[Codec] {
        &self.supported_codecs
    }

    fn build(
        mut self: Box<Self>,
        codec_params: &RTCRtpCodecParameters,
        context: &TrackLocalContext,
    ) -> Box<dyn Encoder> {
        let screen_duplicator =
            match ScreenDuplicator::new(self.device, self.display_index, &self.display_formats) {
                Ok(duplicator) => duplicator,
                Err(e) => {
                    log::error!("{e}");
                    panic!("Failed to create `ScreenDuplicator`");
                }
            };

        let codec = {
            match codec_params.capability.mime_type.as_str() {
                "video/H264" => nvenc::Codec::H264,
                "video/H265" => nvenc::Codec::Hevc,
                "video/AV1" => todo!("AV1 is not supported by the nvenc version used"),
                _ => panic!("Unsupported codec"),
            }
        };
        let profile = nvenc::CodecProfile::H264High;

        if let Err(e) = self.inner_builder.with_codec(codec) {
            log::error!("{e}");
            panic!("Encoder does not support the codec `{codec:?}`");
        }

        let supported_encode_presets = match self.inner_builder.supported_encode_presets(codec) {
            Ok(supported_encode_presets) => supported_encode_presets,
            Err(e) => {
                log::error!("{e}");
                panic!("Failed to query encode presets");
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
            log::error!("{e}");
            panic!("Error configuring encoder");
        }

        let (width, height, texture_format, refresh_ratio) = {
            let display_desc = screen_duplicator.desc();
            let mode_desc = &display_desc.ModeDesc;
            let refresh_ratio = (
                mode_desc.RefreshRate.Numerator,
                mode_desc.RefreshRate.Denominator,
            );
            (
                mode_desc.Width,
                mode_desc.Height,
                mode_desc.Format,
                refresh_ratio,
            )
        };

        let (input, output) =
            match self
                .inner_builder
                .build(width, height, texture_format, None, refresh_ratio)
            {
                Ok((input, output)) => (input, output),
                Err(e) => {
                    log::error!("{e}");
                    panic!("Failed to build encoder");
                }
            };

        Box::new(super::NvidiaEncoder::new(
            screen_duplicator,
            self.display_formats,
            input,
            output,
            codec_params.payload_type,
            context.ssrc(),
        ))
    }
}

fn list_supported_codecs(
    inner_builder: &mut nvenc::EncoderBuilder<nvenc::DirectX11Device>,
) -> nvenc::Result<Vec<Codec>> {
    let mut codecs = Vec::new();
    for codec in inner_builder.supported_codecs()? {
        match codec {
            nvenc::Codec::H264 => {
                let sorter = HashMap::from([
                    (nvenc::CodecProfile::H264High, 0),
                    (nvenc::CodecProfile::H264Main, 1),
                    (nvenc::CodecProfile::H264Baseline, 2),
                ]);

                let mut supported_codec_profiles = inner_builder.supported_codec_profiles(codec)?;
                supported_codec_profiles.sort_by(|a, b| match (sorter.get(a), sorter.get(b)) {
                    (Some(x), Some(y)) => x.cmp(y),
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                });

                for profile in supported_codec_profiles {
                    match profile {
                        nvenc::CodecProfile::H264Baseline => {
                            codecs.push(Codec::h264_custom(66, 0, None));
                            // TODO: Constrained Baseline profile
                            // codecs.push(Codec::h264_custom(66, 0xe0, None));
                        }
                        nvenc::CodecProfile::H264Main => {
                            codecs.push(Codec::h264_custom(77, 0, None));
                        }
                        nvenc::CodecProfile::H264High => {
                            codecs.push(Codec::h264_custom(100, 0, None));
                        }
                        // -- Unimplemented --
                        // nvenc::CodecProfile::H264High444 => todo!(),
                        // nvenc::CodecProfile::H264Stereo => todo!(),
                        // nvenc::CodecProfile::H264ProgressiveHigh => todo!(),
                        // nvenc::CodecProfile::H264ConstrainedHigh => todo!(),
                        nvenc::CodecProfile::HevcMain
                        | nvenc::CodecProfile::HevcMain10
                        | nvenc::CodecProfile::HevcFrext => {
                            panic!("Unexpected HEVC profile returned while using H264 codec")
                        }
                        nvenc::CodecProfile::Autoselect => continue, // Always present
                        _ => continue,                               // Unknown profiles
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

impl NvidiaEncoderBuilder {
    pub fn new() -> Option<Self> {
        let device = create_d3d11_device().ok()?;
        let mut inner_builder = nvenc::EncoderBuilder::new(device.clone()).ok()?;

        let id = INCREMENTAL_ID.fetch_add(1, Ordering::AcqRel);
        let display_index = 0; // default to the first; could be changed later
        let display_formats = vec![
            DXGI_FORMAT_B8G8R8A8_UNORM,
            DXGI_FORMAT_R10G10B10A2_UNORM,
            DXGI_FORMAT_R8G8B8A8_UNORM,
        ];
        let supported_codecs = match list_supported_codecs(&mut inner_builder) {
            Ok(supported_codecs) => supported_codecs,
            Err(e) => {
                log::error!("{e}");
                return None;
            }
        };

        Some(Self {
            inner_builder,
            device,
            id: format!("{}", id),
            display_index,
            display_formats,
            supported_codecs,
        })
    }

    pub fn set_display_index(&mut self, display_index: u32) {
        self.display_index = display_index;
    }
}

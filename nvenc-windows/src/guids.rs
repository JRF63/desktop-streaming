use nvenc_sys;

pub const NV_ENC_CODEC_H264_GUID: nvenc_sys::GUID =
    guid_from_u128(0x6BC82762_4E63_4ca4_AA85_1E50F321F6BF);

pub const NV_ENC_CODEC_HEVC_GUID: nvenc_sys::GUID =
    guid_from_u128(0x790CDC88_4522_4d7b_9425_BDA9975F7603);

pub const NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID: nvenc_sys::GUID =
    guid_from_u128(0xBFD6F8E7_233C_4341_8B3E_4818523803F4);

pub const NV_ENC_H264_PROFILE_BASELINE_GUID: nvenc_sys::GUID =
    guid_from_u128(0x0727BCAA_78C4_4c83_8C2F_EF3DFF267C6A);

pub const NV_ENC_H264_PROFILE_MAIN_GUID: nvenc_sys::GUID =
    guid_from_u128(0x60B5C1D4_67FE_4790_94D5_C4726D7B6E6D);

pub const NV_ENC_H264_PROFILE_HIGH_GUID: nvenc_sys::GUID =
    guid_from_u128(0xE7CBC309_4F7A_4b89_AF2A_D537C92BE310);

pub const NV_ENC_H264_PROFILE_HIGH_444_GUID: nvenc_sys::GUID =
    guid_from_u128(0x7AC663CB_A598_4960_B844_339B261A7D52);

pub const NV_ENC_H264_PROFILE_STEREO_GUID: nvenc_sys::GUID =
    guid_from_u128(0x40847BF5_33F7_4601_9084_E8FE3C1DB8B7);

pub const NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID: nvenc_sys::GUID =
    guid_from_u128(0xB405AFAC_F32B_417B_89C4_9ABEED3E5978);

pub const NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID: nvenc_sys::GUID =
    guid_from_u128(0xAEC1BD87_E85B_48f2_84C3_98BCA6285072);

pub const NV_ENC_HEVC_PROFILE_MAIN_GUID: nvenc_sys::GUID =
    guid_from_u128(0xB514C39A_B55B_40fa_878F_F1253B4DFDEC);

pub const NV_ENC_HEVC_PROFILE_MAIN10_GUID: nvenc_sys::GUID =
    guid_from_u128(0xfa4d2b6c_3a5b_411a_8018_0a3f5e3c9be5);

pub const NV_ENC_HEVC_PROFILE_FREXT_GUID: nvenc_sys::GUID =
    guid_from_u128(0x51ec32b5_1b4c_453c_9cbd_b616bd621341);

pub const NV_ENC_PRESET_P1_GUID: nvenc_sys::GUID =
    guid_from_u128(0xfc0a8d3e_45f8_4cf8_80c7_298871590ebf);

pub const NV_ENC_PRESET_P2_GUID: nvenc_sys::GUID =
    guid_from_u128(0xf581cfb8_88d6_4381_93f0_df13f9c27dab);

pub const NV_ENC_PRESET_P3_GUID: nvenc_sys::GUID =
    guid_from_u128(0x36850110_3a07_441f_94d5_3670631f91f6);

pub const NV_ENC_PRESET_P4_GUID: nvenc_sys::GUID =
    guid_from_u128(0x90a7b826_df06_4862_b9d2_cd6d73a08681);

pub const NV_ENC_PRESET_P5_GUID: nvenc_sys::GUID =
    guid_from_u128(0x21c6e6b4_297a_4cba_998f_b6cbde72ade3);

pub const NV_ENC_PRESET_P6_GUID: nvenc_sys::GUID =
    guid_from_u128(0x8e75c279_6299_4ab6_8302_0b215a335cf5);

pub const NV_ENC_PRESET_P7_GUID: nvenc_sys::GUID =
    guid_from_u128(0x84848c12_6f71_4c13_931b_53e283f57974);

pub const fn guid_from_u128(uuid: u128) -> nvenc_sys::GUID {
    nvenc_sys::GUID {
        Data1: (uuid >> 96) as u32,
        Data2: (uuid >> 80 & 0xffff) as u16,
        Data3: (uuid >> 64 & 0xffff) as u16,
        Data4: (uuid as u64).to_be_bytes(),
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Codec {
    H264,
    Hevc,
}

impl Into<nvenc_sys::GUID> for Codec {
    fn into(self) -> nvenc_sys::GUID {
        match self {
            H264 => NV_ENC_CODEC_H264_GUID,
            Hevc => NV_ENC_CODEC_HEVC_GUID,
        }
    }
}

impl From<nvenc_sys::GUID> for Codec {
    fn from(guid: nvenc_sys::GUID) -> Self {
        if guid == NV_ENC_CODEC_H264_GUID {
            Codec::H264
        } else if guid == NV_ENC_CODEC_HEVC_GUID {
            Codec::Hevc
        } else {
            panic!("Invalid codec guid.")
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum CodecProfile {
    Autoselect,
    H264Baseline,
    H264Main,
    H264High,
    H264High444,
    H264Stereo,
    H264ProgressiveHigh,
    H264ConstrainedHigh,
    HevcMain,
    HevcMain10,
    HevcFrext,
}

impl Into<nvenc_sys::GUID> for CodecProfile {
    fn into(self) -> nvenc_sys::GUID {
        match self {
            Autoselect => NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID,
            H264Baseline => NV_ENC_H264_PROFILE_BASELINE_GUID,
            H264Main => NV_ENC_H264_PROFILE_MAIN_GUID,
            H264High => NV_ENC_H264_PROFILE_HIGH_GUID,
            H264High444 => NV_ENC_H264_PROFILE_HIGH_444_GUID,
            H264Stereo => NV_ENC_H264_PROFILE_STEREO_GUID,
            H264ProgressiveHigh => NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID,
            H264ConstrainedHigh => NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID,
            HevcMain => NV_ENC_HEVC_PROFILE_MAIN_GUID,
            HevcMain10 => NV_ENC_HEVC_PROFILE_MAIN10_GUID,
            HevcFrext => NV_ENC_HEVC_PROFILE_FREXT_GUID,
        }
    }
}

impl From<nvenc_sys::GUID> for CodecProfile {
    fn from(guid: nvenc_sys::GUID) -> Self {
        if guid == NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID {
            CodecProfile::Autoselect
        } else if guid == NV_ENC_H264_PROFILE_BASELINE_GUID {
            CodecProfile::H264Baseline
        } else if guid == NV_ENC_H264_PROFILE_MAIN_GUID {
            CodecProfile::H264Main
        } else if guid == NV_ENC_H264_PROFILE_HIGH_GUID {
            CodecProfile::H264High
        } else if guid == NV_ENC_H264_PROFILE_HIGH_444_GUID {
            CodecProfile::H264High444
        } else if guid == NV_ENC_H264_PROFILE_STEREO_GUID {
            CodecProfile::H264Stereo
        } else if guid == NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID {
            CodecProfile::H264ProgressiveHigh
        } else if guid == NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID {
            CodecProfile::H264ConstrainedHigh
        } else if guid == NV_ENC_HEVC_PROFILE_MAIN_GUID {
            CodecProfile::HevcMain
        } else if guid == NV_ENC_HEVC_PROFILE_MAIN10_GUID {
            CodecProfile::HevcMain10
        } else if guid == NV_ENC_HEVC_PROFILE_FREXT_GUID {
            CodecProfile::HevcFrext
        } else {
            panic!("Invalid codec profile guid.")
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum EncoderPreset {
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
}

impl Into<nvenc_sys::GUID> for EncoderPreset {
    fn into(self) -> nvenc_sys::GUID {
        match self {
            P1 => NV_ENC_PRESET_P1_GUID,
            P2 => NV_ENC_PRESET_P2_GUID,
            P3 => NV_ENC_PRESET_P3_GUID,
            P4 => NV_ENC_PRESET_P4_GUID,
            P5 => NV_ENC_PRESET_P5_GUID,
            P6 => NV_ENC_PRESET_P6_GUID,
            P7 => NV_ENC_PRESET_P7_GUID,
        }
    }
}

impl From<nvenc_sys::GUID> for EncoderPreset {
    fn from(guid: nvenc_sys::GUID) -> Self {
        if guid == NV_ENC_PRESET_P1_GUID {
            EncoderPreset::P1
        } else if guid == NV_ENC_PRESET_P2_GUID {
            EncoderPreset::P2
        } else if guid == NV_ENC_PRESET_P3_GUID {
            EncoderPreset::P3
        } else if guid == NV_ENC_PRESET_P4_GUID {
            EncoderPreset::P4
        } else if guid == NV_ENC_PRESET_P5_GUID {
            EncoderPreset::P5
        } else if guid == NV_ENC_PRESET_P6_GUID {
            EncoderPreset::P6
        } else if guid == NV_ENC_PRESET_P7_GUID {
            EncoderPreset::P7
        } else {
            panic!("Invalid encoder preset.")
        }
    }
}

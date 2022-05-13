use crate::ffi;

const NV_ENC_CODEC_H264_GUID: ffi::GUID = guid_from_u128(0x6BC82762_4E63_4ca4_AA85_1E50F321F6BF);

const NV_ENC_CODEC_HEVC_GUID: ffi::GUID = guid_from_u128(0x790CDC88_4522_4d7b_9425_BDA9975F7603);

const NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID: ffi::GUID =
    guid_from_u128(0xBFD6F8E7_233C_4341_8B3E_4818523803F4);

const NV_ENC_H264_PROFILE_BASELINE_GUID: ffi::GUID =
    guid_from_u128(0x0727BCAA_78C4_4c83_8C2F_EF3DFF267C6A);

const NV_ENC_H264_PROFILE_MAIN_GUID: ffi::GUID =
    guid_from_u128(0x60B5C1D4_67FE_4790_94D5_C4726D7B6E6D);

const NV_ENC_H264_PROFILE_HIGH_GUID: ffi::GUID =
    guid_from_u128(0xE7CBC309_4F7A_4b89_AF2A_D537C92BE310);

const NV_ENC_H264_PROFILE_HIGH_444_GUID: ffi::GUID =
    guid_from_u128(0x7AC663CB_A598_4960_B844_339B261A7D52);

const NV_ENC_H264_PROFILE_STEREO_GUID: ffi::GUID =
    guid_from_u128(0x40847BF5_33F7_4601_9084_E8FE3C1DB8B7);

const NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID: ffi::GUID =
    guid_from_u128(0xB405AFAC_F32B_417B_89C4_9ABEED3E5978);

const NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID: ffi::GUID =
    guid_from_u128(0xAEC1BD87_E85B_48f2_84C3_98BCA6285072);

const NV_ENC_HEVC_PROFILE_MAIN_GUID: ffi::GUID =
    guid_from_u128(0xB514C39A_B55B_40fa_878F_F1253B4DFDEC);

const NV_ENC_HEVC_PROFILE_MAIN10_GUID: ffi::GUID =
    guid_from_u128(0xfa4d2b6c_3a5b_411a_8018_0a3f5e3c9be5);

const NV_ENC_HEVC_PROFILE_FREXT_GUID: ffi::GUID =
    guid_from_u128(0x51ec32b5_1b4c_453c_9cbd_b616bd621341);

const fn guid_from_u128(uuid: u128) -> ffi::GUID {
    ffi::GUID {
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

impl Codec {
    pub(crate) fn to_guid(self) -> ffi::GUID {
        match self {
            Codec::H264 => NV_ENC_CODEC_H264_GUID,
            Codec::Hevc => NV_ENC_CODEC_HEVC_GUID,
        }
    }

    pub(crate) fn from_guid(guid: ffi::GUID) -> Self {
        if guid == NV_ENC_CODEC_H264_GUID {
            Codec::H264
        } else if guid == NV_ENC_CODEC_HEVC_GUID {
            Codec::Hevc
        } else {
            panic!("Invalid codec guid")
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

impl CodecProfile {
    pub(crate) fn to_guid(self) -> ffi::GUID {
        match self {
            CodecProfile::Autoselect => NV_ENC_CODEC_PROFILE_AUTOSELECT_GUID,
            CodecProfile::H264Baseline => NV_ENC_H264_PROFILE_BASELINE_GUID,
            CodecProfile::H264Main => NV_ENC_H264_PROFILE_MAIN_GUID,
            CodecProfile::H264High => NV_ENC_H264_PROFILE_HIGH_GUID,
            CodecProfile::H264High444 => NV_ENC_H264_PROFILE_HIGH_444_GUID,
            CodecProfile::H264Stereo => NV_ENC_H264_PROFILE_STEREO_GUID,
            CodecProfile::H264ProgressiveHigh => NV_ENC_H264_PROFILE_PROGRESSIVE_HIGH_GUID,
            CodecProfile::H264ConstrainedHigh => NV_ENC_H264_PROFILE_CONSTRAINED_HIGH_GUID,
            CodecProfile::HevcMain => NV_ENC_HEVC_PROFILE_MAIN_GUID,
            CodecProfile::HevcMain10 => NV_ENC_HEVC_PROFILE_MAIN10_GUID,
            CodecProfile::HevcFrext => NV_ENC_HEVC_PROFILE_FREXT_GUID,
        }
    }

    pub(crate) fn from_guid(guid: ffi::GUID) -> Self {
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
            panic!("Invalid codec profile guid")
        }
    }
}

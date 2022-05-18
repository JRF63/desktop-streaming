use nvenc_sys;

pub const NV_ENC_CODEC_H264_GUID: nvenc_sys::GUID = guid_from_u128(0x6BC82762_4E63_4ca4_AA85_1E50F321F6BF);

pub const NV_ENC_CODEC_HEVC_GUID: nvenc_sys::GUID = guid_from_u128(0x790CDC88_4522_4d7b_9425_BDA9975F7603);

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

pub const fn guid_from_u128(uuid: u128) -> nvenc_sys::GUID {
    nvenc_sys::GUID {
        Data1: (uuid >> 96) as u32,
        Data2: (uuid >> 80 & 0xffff) as u16,
        Data3: (uuid >> 64 & 0xffff) as u16,
        Data4: (uuid as u64).to_be_bytes(),
    }
}

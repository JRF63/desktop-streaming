#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

const fn nvencapi_struct_version(ver: u32) -> u32 {
    NVENCAPI_VERSION | (ver << 16) | (0x7 << 28)
}

pub const NV_ENC_CAPS_PARAM_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_ENCODE_OUT_PARAMS_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_CREATE_INPUT_BUFFER_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_CREATE_BITSTREAM_BUFFER_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_CREATE_MV_BUFFER_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_RC_PARAMS_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_CONFIG_VER: u32 = nvencapi_struct_version(7) | (1 << 31);
pub const NV_ENC_INITIALIZE_PARAMS_VER: u32 = nvencapi_struct_version(5) | (1 << 31);
pub const NV_ENC_RECONFIGURE_PARAMS_VER: u32 = nvencapi_struct_version(1) | (1 << 31);
pub const NV_ENC_PRESET_CONFIG_VER: u32 = nvencapi_struct_version(4) | (1 << 31);
pub const NV_ENC_PIC_PARAMS_MVC_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_PIC_PARAMS_VER: u32 = nvencapi_struct_version(4) | (1 << 31);
pub const NV_ENC_MEONLY_PARAMS_VER: u32 = nvencapi_struct_version(3);
pub const NV_ENC_LOCK_BITSTREAM_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_LOCK_INPUT_BUFFER_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_MAP_INPUT_RESOURCE_VER: u32 = nvencapi_struct_version(4);
pub const NV_ENC_REGISTER_RESOURCE_VER: u32 = nvencapi_struct_version(3);
pub const NV_ENC_STAT_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_SEQUENCE_PARAM_PAYLOAD_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_EVENT_PARAMS_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENCODE_API_FUNCTION_LIST_VER: u32 = nvencapi_struct_version(2);

impl PartialEq for GUID {
    fn eq(&self, other: &Self) -> bool {
        self.Data1 == other.Data1
            && self.Data2 == other.Data2
            && self.Data3 == other.Data3
            && self.Data4 == other.Data4
    }
}

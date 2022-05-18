#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

const fn nvencapi_struct_version(ver: u32) -> u32 {
    NVENCAPI_VERSION | ((ver) << 16) | (0x7 << 28)
}

pub const NV_ENCODE_API_FUNCTION_LIST_VER: u32 = nvencapi_struct_version(2);
pub const NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_REGISTER_RESOURCE_VER: u32 = nvencapi_struct_version(5);
pub const NV_ENC_CREATE_BITSTREAM_BUFFER_VER: u32 = nvencapi_struct_version(1);
pub const NV_ENC_MAP_INPUT_RESOURCE_VER: u32 = nvencapi_struct_version(4);
pub const NV_ENC_LOCK_BITSTREAM_VER: u32 = nvencapi_struct_version(1);

impl PartialEq for GUID {
    fn eq(&self, other: &Self) -> bool {
        self.Data1 == other.Data1 && self.Data2 == other.Data2 && self.Data3 == other.Data3 && self.Data4 == other.Data4
    }
}
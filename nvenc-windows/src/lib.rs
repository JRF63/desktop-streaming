mod error;
mod ffi;
mod guids;
mod util;

pub use guids::*;
use std::{mem::MaybeUninit, os::raw::c_void, ptr::NonNull};

macro_rules! nvenc_function {
    ($fn:expr, $($arg:expr),*) => {
        let status = ($fn.unwrap_or_else(|| std::hint::unreachable_unchecked()))($($arg,)*);
        if status != ffi::NVENCSTATUS::NV_ENC_SUCCESS {
            return Err(error::NvEncError::new(status));
        }
    }
}

pub type Result<T> = std::result::Result<T, error::NvEncError>;

pub struct NvEnc {
    encoder: NonNull<c_void>,
    functions: ffi::NV_ENCODE_API_FUNCTION_LIST,
    // _lib: HINSTANCE
}

impl NvEnc {
    pub fn new(device: NonNull<c_void>, device_type: ffi::NV_ENC_DEVICE_TYPE) -> Self {
        let functions = util::load_nvenc_library().unwrap();
        let encoder = util::open_encode_session(&functions, device, device_type).unwrap();
        Self { encoder, functions }
    }

    pub fn get_codec_profiles(&self, codec: Codec) -> Result<Vec<CodecProfile>> {
        let encode_guid = codec.to_guid();
        let profile_guids = self.get_encode_profile_guids(encode_guid)?;
        let encode_profiles = profile_guids
            .iter()
            .map(|guid| CodecProfile::from_guid(*guid))
            .collect();
        Ok(encode_profiles)
    }

    pub fn get_codecs(&self) -> Result<Vec<Codec>> {
        let encode_guids = self.get_encode_guids()?;
        let codecs = encode_guids
            .iter()
            .map(|guid| Codec::from_guid(*guid))
            .collect();
        Ok(codecs)
    }

    fn get_encode_guid_count(&self) -> Result<u32> {
        let mut encode_guid_count = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeGUIDCount,
                self.encoder.as_ptr(),
                encode_guid_count.as_mut_ptr()
            );
            Ok(encode_guid_count.assume_init())
        }
    }

    fn get_encode_profile_guid_count(&self, encode_guid: ffi::GUID) -> Result<u32> {
        let mut encode_profile_guid_count = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeProfileGUIDCount,
                self.encoder.as_ptr(),
                encode_guid,
                encode_profile_guid_count.as_mut_ptr()
            );
            Ok(encode_profile_guid_count.assume_init())
        }
    }

    fn get_encode_profile_guids(&self, encode_guid: ffi::GUID) -> Result<Vec<ffi::GUID>> {
        let encode_profile_guid_count = self.get_encode_profile_guid_count(encode_guid)?;
        let mut profile_guids = Vec::with_capacity(encode_profile_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeProfileGUIDs,
                self.encoder.as_ptr(),
                encode_guid,
                profile_guids.as_mut_ptr(),
                encode_profile_guid_count,
                num_entries.as_mut_ptr()
            );
            profile_guids.set_len(num_entries.assume_init() as usize);
        }
        Ok(profile_guids)
    }

    fn get_encode_guids(&self) -> Result<Vec<ffi::GUID>> {
        let encode_guid_count = self.get_encode_guid_count()?;
        let mut encode_guids = Vec::with_capacity(encode_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeGUIDs,
                self.encoder.as_ptr(),
                encode_guids.as_mut_ptr(),
                encode_guid_count,
                num_entries.as_mut_ptr()
            );
            encode_guids.set_len(num_entries.assume_init() as usize);
        }
        Ok(encode_guids)
    }
}

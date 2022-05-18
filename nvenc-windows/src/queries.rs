use crate::{
    guids::{Codec, CodecProfile},
    nvenc_function, NvidiaEncoder, Result,
};
use std::mem::MaybeUninit;

impl<const BUF_SIZE: usize> NvidiaEncoder<BUF_SIZE> {
    pub fn codec_profiles(&self, codec: Codec) -> Result<Vec<CodecProfile>> {
        let encode_guid = codec.to_guid();
        let profile_guids = self.encode_profile_guids(encode_guid)?;
        let encode_profiles = profile_guids
            .iter()
            .map(|guid| CodecProfile::from_guid(*guid))
            .collect();
        Ok(encode_profiles)
    }

    pub fn codecs(&self) -> Result<Vec<Codec>> {
        let encode_guids = self.encode_guids()?;
        let codecs = encode_guids
            .iter()
            .map(|guid| Codec::from_guid(*guid))
            .collect();
        Ok(codecs)
    }

    fn encode_guid_count(&self) -> Result<u32> {
        let mut encode_guid_count = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeGUIDCount,
                self.raw_encoder.as_ptr(),
                encode_guid_count.as_mut_ptr()
            );
            Ok(encode_guid_count.assume_init())
        }
    }

    fn encode_profile_guid_count(&self, encode_guid: nvenc_sys::GUID) -> Result<u32> {
        let mut encode_profile_guid_count = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeProfileGUIDCount,
                self.raw_encoder.as_ptr(),
                encode_guid,
                encode_profile_guid_count.as_mut_ptr()
            );
            Ok(encode_profile_guid_count.assume_init())
        }
    }

    fn encode_profile_guids(&self, encode_guid: nvenc_sys::GUID) -> Result<Vec<nvenc_sys::GUID>> {
        let encode_profile_guid_count = self.encode_profile_guid_count(encode_guid)?;
        let mut profile_guids = Vec::with_capacity(encode_profile_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeProfileGUIDs,
                self.raw_encoder.as_ptr(),
                encode_guid,
                profile_guids.as_mut_ptr(),
                encode_profile_guid_count,
                num_entries.as_mut_ptr()
            );
            profile_guids.set_len(num_entries.assume_init() as usize);
        }
        Ok(profile_guids)
    }

    fn encode_guids(&self) -> Result<Vec<nvenc_sys::GUID>> {
        let encode_guid_count = self.encode_guid_count()?;
        let mut encode_guids = Vec::with_capacity(encode_guid_count as usize);
        let mut num_entries = MaybeUninit::uninit();
        unsafe {
            nvenc_function!(
                self.functions.nvEncGetEncodeGUIDs,
                self.raw_encoder.as_ptr(),
                encode_guids.as_mut_ptr(),
                encode_guid_count,
                num_entries.as_mut_ptr()
            );
            encode_guids.set_len(num_entries.assume_init() as usize);
        }
        Ok(encode_guids)
    }
}

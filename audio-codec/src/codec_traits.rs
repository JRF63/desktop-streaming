use crate::sys;

/// Audio data that can be encoded.
pub trait Encodeable {
    unsafe fn encode(
        st: *mut sys::OpusEncoder,
        input: *const Self,
        num_frames: ::std::os::raw::c_int,
        output: *mut ::std::os::raw::c_uchar,
        max_output_bytes: i32,
    ) -> i32;
}

/// Audio data that can be decoded.
pub trait Decodeable {
    unsafe fn decode(
        st: *mut sys::OpusDecoder,
        input: *const ::std::os::raw::c_uchar,
        input_num_bytes: i32,
        output: *mut Self,
        max_num_frames: ::std::os::raw::c_int,
        decode_fec: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
}

macro_rules! impl_codec_traits {
    ($t:ty, $encode_fn:expr, $decode_fn:expr) => {
        impl Encodeable for $t {
            #[inline]
            unsafe fn encode(
                st: *mut sys::OpusEncoder,
                input: *const Self,
                num_frames: std::os::raw::c_int,
                output: *mut std::os::raw::c_uchar,
                max_output_bytes: i32,
            ) -> i32 {
                $encode_fn(st, input, num_frames, output, max_output_bytes)
            }
        }

        impl Decodeable for $t {
            #[inline]
            unsafe fn decode(
                st: *mut sys::OpusDecoder,
                input: *const ::std::os::raw::c_uchar,
                input_num_bytes: i32,
                output: *mut Self,
                max_num_frames: ::std::os::raw::c_int,
                decode_fec: ::std::os::raw::c_int,
            ) -> ::std::os::raw::c_int {
                $decode_fn(
                    st,
                    input,
                    input_num_bytes,
                    output,
                    max_num_frames,
                    decode_fec,
                )
            }
        }
    };
}

impl_codec_traits!(i16, sys::opus_encode, sys::opus_decode);
impl_codec_traits!(f32, sys::opus_encode_float, sys::opus_decode_float);

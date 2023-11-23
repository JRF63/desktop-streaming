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

macro_rules! impl_encodeable {
    ($t:ty, $func:expr) => {
        impl Encodeable for $t {
            #[inline]
            unsafe fn encode(
                st: *mut sys::OpusEncoder,
                input: *const Self,
                num_frames: std::os::raw::c_int,
                output: *mut std::os::raw::c_uchar,
                max_output_bytes: i32,
            ) -> i32 {
                $func(st, input, num_frames, output, max_output_bytes)
            }
        }
    };
}

impl_encodeable!(i16, sys::opus_encode);
impl_encodeable!(f32, sys::opus_encode_float);

macro_rules! impl_decodeable {
    ($t:ty, $func:expr) => {
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
                $func(
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

impl_decodeable!(i16, sys::opus_decode);
impl_decodeable!(f32, sys::opus_decode_float);

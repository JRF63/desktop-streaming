use std::os::raw::c_void;

pub(crate) trait IntoNvEncBufferFormat {
    fn into_nvenc_buffer_format(&self) -> crate::sys::NV_ENC_BUFFER_FORMAT;
}

pub(crate) trait NvEncDevice {
    fn device_type() -> crate::sys::NV_ENC_DEVICE_TYPE;

    fn as_ptr(&self) -> *mut c_void;
}

pub(crate) trait NvEncTexture {
    type Format: IntoNvEncBufferFormat;

    fn resource_type() -> crate::sys::NV_ENC_INPUT_RESOURCE_TYPE;

    /// Returns (width, height, texture_format)
    fn desc(&self) -> (u32, u32, Self::Format);

    fn as_ptr(&self) -> *mut c_void;
}

pub(crate) trait NvEncSessionParameters {
    type Format: IntoNvEncBufferFormat;

    /// Returns (encode width, encode height)
    fn resolution(&self) -> (u32, u32);

    fn display_aspect_ratio(&self) -> (u32, u32) {
        let (width, height) = self.resolution();
        let divisor = gcd(width, height);
        (width / divisor, height / divisor)
    }
}

// https://en.wikipedia.org/wiki/Binary_GCD_algorithm
pub(crate) fn gcd(mut u: u32, mut v: u32) -> u32 {
    use std::cmp::min;
    use std::mem::swap;

    if u == 0 {
        return v;
    } else if v == 0 {
        return u;
    }

    let i = u.trailing_zeros();
    u >>= i;
    let j = v.trailing_zeros();
    v >>= j;
    let k = min(i, j);

    loop {
        if u > v {
            swap(&mut u, &mut v);
        }
        v -= u;
        if v == 0 {
            return u << k;
        }
        v >>= v.trailing_zeros();
    }
}

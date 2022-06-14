use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R10G10B10A2_UNORM,
    DXGI_FORMAT_R8G8B8A8_UNORM,
};

pub(crate) fn dxgi_to_nv_format(format: DXGI_FORMAT) -> nvenc_sys::NV_ENC_BUFFER_FORMAT {
    match format {
        DXGI_FORMAT_B8G8R8A8_UNORM => nvenc_sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_ARGB,
        DXGI_FORMAT_R10G10B10A2_UNORM => {
            nvenc_sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_ABGR10
        }
        DXGI_FORMAT_R8G8B8A8_UNORM => nvenc_sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_ABGR,
        _ => nvenc_sys::NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_UNDEFINED,
    }
}

pub(crate) trait IntoNvEncBufferFormat {
    fn into_nvenc_buffer_format(&self) -> nvenc_sys::NV_ENC_BUFFER_FORMAT;
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
use ndk_sys::{clock_gettime, timespec, CLOCK_MONOTONIC};
use std::os::raw::c_int;

pub(crate) fn system_nanotime() -> u64 {
    let mut now = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        let _ignored = clock_gettime(CLOCK_MONOTONIC as c_int, &mut now);
    }
    (now.tv_sec as u64)
        .wrapping_mul(1_000_000_000)
        .wrapping_add(now.tv_nsec as u64)
}

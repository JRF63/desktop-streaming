pub(crate) const TAG: &'static str = "client-android\0";

#[macro_export]
macro_rules! info {
    ($($arg:expr),+) => {
        crate::log_write!(ndk_sys::android_LogPriority_ANDROID_LOG_INFO, $($arg),+)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:expr),+) => {
        crate::log_write!(ndk_sys::android_LogPriority_ANDROID_LOG_ERROR, $($arg),+)
    };
}

#[macro_export]
macro_rules! log_write {
    ($prio:expr, $($arg:expr),+) => {
        {
            let mut s = format!($($arg),+);
            s.push('\0');
            #[allow(unused_unsafe)]
            unsafe {
                ndk_sys::__android_log_write(
                    $prio as i32,
                    crate::log::TAG.as_ptr().cast(),
                    s.as_ptr().cast()
                );
            }
        }
    };
}
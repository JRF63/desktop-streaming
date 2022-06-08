use jni::{objects::JObject, JNIEnv};
use ndk_sys::{ANativeWindow, ANativeWindow_fromSurface, ANativeWindow_release};
use std::ptr::NonNull;

#[repr(transparent)]
pub(crate) struct NativeWindow(NonNull<ANativeWindow>);

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            ANativeWindow_release(self.0.as_ptr());
        }
    }
}

impl NativeWindow {
    pub(crate) fn new(env: &JNIEnv, surface: &JObject) -> Option<Self> {
        NonNull::new(unsafe {
            ANativeWindow_fromSurface(env.get_native_interface(), surface.into_inner())
        })
        .map(|ptr| NativeWindow(ptr))
    }

    pub(crate) fn as_inner(&self) -> *mut ANativeWindow {
        self.0.as_ptr()
    }
}

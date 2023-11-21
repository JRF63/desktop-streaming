use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT};

pub struct ComThread;

impl Drop for ComThread {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

impl ComThread {
    /// Initialize COM on the current thread.
    pub fn new(thread_model: COINIT) -> Result<Self, windows::core::Error> {
        unsafe {
            CoInitializeEx(None, thread_model)?;
            Ok(Self)
        }
    }
}

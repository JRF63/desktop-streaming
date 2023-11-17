use std::mem::MaybeUninit;
use windows::{
    core::HSTRING,
    Win32::{
        Foundation::HANDLE,
        System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW},
    },
};

const PRO_AUDIO_TASK_NAME: &str = "Pro Audio";

pub struct ThreadPriorityBooster(HANDLE);

impl Drop for ThreadPriorityBooster {
    fn drop(&mut self) {
        unsafe {
            let _ = AvRevertMmThreadCharacteristics(self.0);
        }
    }
}

impl ThreadPriorityBooster {
    pub fn new() -> Result<Self, windows::core::Error> {
        unsafe {
            let mut task_index = MaybeUninit::uninit();
            let handle = AvSetMmThreadCharacteristicsW(
                &HSTRING::from(PRO_AUDIO_TASK_NAME),
                task_index.as_mut_ptr(),
            )?;
            Ok(Self(handle))
        }
    }
}

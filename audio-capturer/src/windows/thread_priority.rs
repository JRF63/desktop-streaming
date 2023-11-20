use std::mem::MaybeUninit;
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::HANDLE,
        System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsA},
    },
};

#[repr(transparent)]
pub struct ThreadPriority(HANDLE);

impl Drop for ThreadPriority {
    fn drop(&mut self) {
        unsafe {
            let _ = AvRevertMmThreadCharacteristics(self.0);
        }
    }
}

impl ThreadPriority {
    pub fn new(thread_profile: ThreadProfile) -> Result<Self, windows::core::Error> {
        unsafe {
            let mut task_index = MaybeUninit::uninit();

            // Need to have a variable to hold the `&'static str` before using the pointer in
            // `PCSTR::from_raw`. Not doing it this way causes `AvSetMmThreadCharacteristicsA` to
            // hang.
            let task_name = thread_profile.task_name_str();
            let pcstr = PCSTR::from_raw(task_name.as_ptr());

            let handle = AvSetMmThreadCharacteristicsA(pcstr, task_index.as_mut_ptr())?;

            Ok(Self(handle))
        }
    }
}

// TODO: Separate into a util crate
#[allow(dead_code)]
pub enum ThreadProfile {
    Audio,
    Capture,
    DisplayPostProcessing, // Since some version of Windows 10
    Distribution,
    Games,
    Playback,
    ProAudio,
    WindowManager,
}

impl ThreadProfile {
    fn task_name_str(&self) -> &'static str {
        match self {
            ThreadProfile::Audio => "Audio\0",
            ThreadProfile::Capture => "Capture\0",
            // DisplayPostProcessing has no space between the words
            ThreadProfile::DisplayPostProcessing => "DisplayPostProcessing\0",
            ThreadProfile::Distribution => "Distribution\0",
            ThreadProfile::Games => "Games\0",
            ThreadProfile::Playback => "Playback\0",
            ThreadProfile::ProAudio => "Pro Audio\0",
            ThreadProfile::WindowManager => "Window Manager\0",
        }
    }
}

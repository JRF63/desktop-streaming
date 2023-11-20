use windows::{
    core::{s, PCSTR},
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
            // Must be zero on initial call
            let mut task_index = 0;

            let handle =
                AvSetMmThreadCharacteristicsA(thread_profile.task_name(), &mut task_index)?;

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
    fn task_name(&self) -> PCSTR {
        match self {
            ThreadProfile::Audio => s!("Audio"),
            ThreadProfile::Capture => s!("Capture"),
            // DisplayPostProcessing has no space between the words
            ThreadProfile::DisplayPostProcessing => s!("DisplayPostProcessing"),
            ThreadProfile::Distribution => s!("Distribution"),
            ThreadProfile::Games => s!("Games"),
            ThreadProfile::Playback => s!("Playback"),
            ThreadProfile::ProAudio => s!("Pro Audio"),
            ThreadProfile::WindowManager => s!("Window Manager"),
        }
    }
}

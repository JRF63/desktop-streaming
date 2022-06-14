use std::ffi::c_void;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        Threading::{CreateEventA, WaitForSingleObject, WAIT_OBJECT_0},
        WindowsProgramming::INFINITE,
    },
};

#[repr(transparent)]
pub(crate) struct EventObject(HANDLE);

impl Drop for EventObject {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

impl EventObject {
    /// Create a Windows Event Object for signaling encoding completion of a frame.
    pub(crate) fn new() -> windows::core::Result<Self> {
        let event = unsafe { CreateEventA(std::ptr::null(), false, false, None) }?;
        Ok(EventObject(event))
    }

    /// Waits forever until the internal Event Object has been signaled.
    pub(crate) fn blocking_wait(&self) -> windows::core::Result<()> {
        unsafe {
            match WaitForSingleObject(self.0, INFINITE) {
                WAIT_OBJECT_0 => Ok(()),
                _ => Err(windows::core::Error::from_win32()),
            }
        }
    }

    /// Casts the `EventObject` as a raw pointer as required by the NvEncAPI structs.
    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.0 .0 as *mut c_void
    }
}

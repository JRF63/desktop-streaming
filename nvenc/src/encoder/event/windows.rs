use super::EventObjectTrait;
use crate::{NvEncError, Result};
use std::ffi::c_void;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::Threading::{CreateEventA, WaitForSingleObject, WAIT_OBJECT_0},
};

#[repr(transparent)]
pub struct EventObject(HANDLE);

impl Drop for EventObject {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

impl EventObjectTrait for EventObject {
    fn new() -> Result<Self> {
        match unsafe { CreateEventA(std::ptr::null(), false, false, None) } {
            Ok(event) => Ok(EventObject(event)),
            Err(_) => Err(NvEncError::EventObjectCreationFailed),
        }
    }

    fn wait(&self) -> Result<()> {
        const TIMEOUT_INTERVAL_MS: u32 = 1000; // Wait for 1 sec
        const WAIT_TIMEOUT: u32 = windows::Win32::Foundation::WAIT_TIMEOUT.0;

        match unsafe { WaitForSingleObject(self.0, TIMEOUT_INTERVAL_MS) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(NvEncError::EventObjectWaitTimeout),
            _ => Err(NvEncError::EventObjectWaitError),
        }
    }

    fn as_ptr(&self) -> *mut c_void {
        self.0 .0 as *mut c_void
    }
}

#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
mod non_windows;

use crate::Result;
use std::ffi::c_void;

#[cfg(windows)]
pub use self::windows::EventObject;
#[cfg(not(windows))]
pub use self::non_windows::EventObject;

pub trait EventObjectTrait: Sized {
    fn new() -> Result<Self>;

    fn wait(&self) -> Result<()>;

    fn as_ptr(&self) -> *mut c_void;
}
pub mod error;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::*;

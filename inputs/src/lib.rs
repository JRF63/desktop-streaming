#[cfg(windows)]
pub mod windows;

pub enum InputType {
    Keyboard,
    Mouse,
    Gamepad,
    Pen,
    Touch,
}

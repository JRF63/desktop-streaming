use windows::{
    core::Result,
    Win32::UI::{
        Controls::{POINTER_FEEDBACK_NONE, CreateSyntheticPointerDevice, HSYNTHETICPOINTERDEVICE},
        WindowsAndMessaging::{PT_PEN, PT_TOUCH, POINTER_INPUT_TYPE},
    },
};

#[derive(Clone, Copy)]
pub enum DeviceType {
    Touch,
    Pen,
}

impl DeviceType {
    fn as_native(self) -> POINTER_INPUT_TYPE {
        match self {
            DeviceType::Touch => PT_TOUCH,
            DeviceType::Pen => PT_PEN,
        }
    }
}

#[repr(transparent)]
pub struct PointerDevice(HSYNTHETICPOINTERDEVICE);

impl PointerDevice {
    pub fn new(device_type: DeviceType) -> Result<Self> {
        let contacts = match device_type {
            DeviceType::Touch => 10, // Humans have 10 fingers
            DeviceType::Pen => 1,
        };
        let device = unsafe {
            CreateSyntheticPointerDevice(
                device_type.as_native(),
                contacts,
                POINTER_FEEDBACK_NONE
            )?
        };
        Ok(PointerDevice(device))
    }
}

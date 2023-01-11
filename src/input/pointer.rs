use serde::{Deserialize, Serialize};
use windows::Win32::{
    Foundation::{HANDLE, HWND, POINT, RECT},
    UI::{
        Controls::{
            CreateSyntheticPointerDevice, DestroySyntheticPointerDevice, HSYNTHETICPOINTERDEVICE,
            POINTER_FEEDBACK_NONE, POINTER_TYPE_INFO, POINTER_TYPE_INFO_0,
        },
        Input::Pointer::{
            InjectSyntheticPointerInput, POINTER_CHANGE_NONE, POINTER_FLAG_CANCELED,
            POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE, POINTER_FLAG_NONE,
            POINTER_FLAG_PRIMARY, POINTER_FLAG_UP, POINTER_FLAG_UPDATE, POINTER_INFO,
            POINTER_PEN_INFO, POINTER_TOUCH_INFO,
        },
        WindowsAndMessaging::{
            PEN_MASK_PRESSURE, PEN_MASK_ROTATION, PEN_MASK_TILT_X, PEN_MASK_TILT_Y,
            POINTER_MOD_CTRL, POINTER_MOD_SHIFT, PT_MOUSE, PT_PEN, PT_TOUCH,
            TOUCH_MASK_CONTACTAREA, TOUCH_MASK_PRESSURE,
        },
    },
};

const MAX_CONTACTS: usize = 10;

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum PointerType {
    #[serde(rename = "mouse")]
    Mouse,
    #[serde(rename = "pen")]
    Pen,
    #[serde(rename = "touch")]
    Touch,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum PointerEventType {
    #[serde(rename = "pointerover")]
    Over,
    #[serde(rename = "pointerenter")]
    Enter,
    #[serde(rename = "pointerdown")]
    Down,
    #[serde(rename = "pointermove")]
    Move,
    #[serde(rename = "pointerrawupdate")]
    RawUpdate,
    #[serde(rename = "pointerup")]
    Up,
    #[serde(rename = "pointercancel")]
    Cancel,
    #[serde(rename = "pointerout")]
    Out,
    #[serde(rename = "pointerleave")]
    Leave,
    #[serde(rename = "gotpointercapture")]
    GotCapture,
    #[serde(rename = "lostpointercapture")]
    LostCapture,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub struct PenExtra {
    #[serde(rename = "tiltX")]
    tilt_x: i32,
    #[serde(rename = "tiltY")]
    tilt_y: i32,
    twist: u32,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub struct ModifierKeys {
    #[serde(rename = "ctrlKey")]
    ctrl_key: bool,
    #[serde(rename = "shiftKey")]
    shift_key: bool,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub struct PointerEvent {
    #[serde(rename = "type")]
    event_type: PointerEventType,
    #[serde(rename = "pointerId")]
    id: u64,
    #[serde(rename = "isPrimary")]
    is_primary: bool,

    x: f64,
    y: f64,
    width: f64,
    height: f64,

    pointer_type: Option<PointerType>,

    pressure: Option<f64>,

    #[serde(rename = "penExtra")]
    pen_extra: Option<PenExtra>,

    #[serde(rename = "modifierKeys")]
    modifier_keys: Option<ModifierKeys>,
}

impl Into<POINTER_TYPE_INFO> for PointerEvent {
    fn into(self) -> POINTER_TYPE_INFO {
        let mut pointer_flags = match self.event_type {
            PointerEventType::Over | PointerEventType::Enter => {
                POINTER_FLAG_INRANGE | POINTER_FLAG_UPDATE
            }
            PointerEventType::Down => {
                POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_DOWN
            }
            PointerEventType::Move => {
                POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_UPDATE
            }
            PointerEventType::Up => POINTER_FLAG_UP,
            PointerEventType::Cancel => POINTER_FLAG_CANCELED,
            PointerEventType::Out | PointerEventType::Leave => POINTER_FLAG_UPDATE,
            PointerEventType::RawUpdate
            | PointerEventType::GotCapture
            | PointerEventType::LostCapture => POINTER_FLAG_NONE, // Unhandled event types
        };

        let device_type = if let Some(pointer_type) = self.pointer_type {
            match pointer_type {
                PointerType::Mouse => PT_MOUSE,
                PointerType::Pen => PT_PEN,
                PointerType::Touch => PT_TOUCH,
            }
        } else {
            PT_TOUCH
        };

        let pointer_id = self.id as u32;
        let frame_id: u32 = 0;

        if self.is_primary {
            pointer_flags |= POINTER_FLAG_PRIMARY;
        }

        let key_states = match self.modifier_keys {
            Some(keys) => {
                let mut key_states = 0;
                if keys.shift_key {
                    key_states |= POINTER_MOD_SHIFT;
                }
                if keys.ctrl_key {
                    key_states |= POINTER_MOD_CTRL;
                }
                key_states
            }
            None => 0,
        };

        let mut touch_mask = TOUCH_MASK_CONTACTAREA;
        let pressure = if let Some(pressure) = self.pressure {
            touch_mask |= TOUCH_MASK_PRESSURE;
            (pressure * 1024.0) as u32
        } else {
            0
        };

        let x = self.x;
        let y = self.y;

        let pointer_info = POINTER_INFO {
            pointerType: device_type,
            pointerId: pointer_id,
            frameId: frame_id,
            pointerFlags: pointer_flags,
            sourceDevice: HANDLE::default(),
            hwndTarget: HWND::default(),
            ptPixelLocation: POINT {
                x: x as i32,
                y: y as i32,
            },
            ptHimetricLocation: POINT::default(),
            ptPixelLocationRaw: POINT::default(),
            ptHimetricLocationRaw: POINT::default(),
            dwTime: 0,       // Unused
            historyCount: 1, // No coalescing
            InputData: 0,    // Undocumented
            dwKeyStates: key_states,
            PerformanceCount: 0, // Must be set outside this function
            ButtonChangeType: POINTER_CHANGE_NONE,
        };

        let union_arg = if device_type == PT_TOUCH {
            let width_half = self.width / 2.0;
            let height_half = self.height / 2.0;
            let contact_area = RECT {
                left: (x - width_half) as i32,
                top: (y - height_half) as i32,
                right: (x + width_half) as i32,
                bottom: (y + height_half) as i32,
            };

            POINTER_TYPE_INFO_0 {
                touchInfo: POINTER_TOUCH_INFO {
                    pointerInfo: pointer_info,
                    touchFlags: 0, // 0 is the only valid value here
                    touchMask: touch_mask,
                    rcContact: contact_area,
                    rcContactRaw: RECT::default(), // TODO
                    orientation: 0,                // TODO
                    pressure,
                },
            }
        } else {
            let mut pen_mask = 0;

            let (twist, tilt_x, tilt_y) = if let Some(pen_extra) = self.pen_extra {
                pen_mask |=
                    PEN_MASK_PRESSURE | PEN_MASK_ROTATION | PEN_MASK_TILT_X | PEN_MASK_TILT_Y;
                (pen_extra.twist, pen_extra.tilt_x, pen_extra.tilt_y)
            } else {
                (0, 0, 0)
            };

            POINTER_TYPE_INFO_0 {
                penInfo: POINTER_PEN_INFO {
                    pointerInfo: pointer_info,
                    penFlags: 0, // TODO: Look at MouseEvent to see if this is in there
                    penMask: pen_mask,
                    pressure,
                    rotation: twist,
                    tiltX: tilt_x,
                    tiltY: tilt_y,
                },
            }
        };

        POINTER_TYPE_INFO {
            r#type: device_type,
            Anonymous: union_arg,
        }
    }
}

pub struct PointerDevice {
    touch: HSYNTHETICPOINTERDEVICE,
    pen: HSYNTHETICPOINTERDEVICE,
}

impl Drop for PointerDevice {
    fn drop(&mut self) {
        unsafe {
            DestroySyntheticPointerDevice(self.touch);
            DestroySyntheticPointerDevice(self.pen);
        }
    }
}

impl PointerDevice {
    pub fn new() -> Result<Self, windows::core::Error> {
        let touch = unsafe {
            CreateSyntheticPointerDevice(PT_TOUCH, MAX_CONTACTS as u32, POINTER_FEEDBACK_NONE)?
        };

        let pen = unsafe { CreateSyntheticPointerDevice(PT_PEN, 1, POINTER_FEEDBACK_NONE)? };

        Ok(PointerDevice { touch, pen })
    }

    pub fn inject_pointer_input(
        &self,
        inputs: &[POINTER_TYPE_INFO],
    ) -> Result<(), windows::core::Error> {
        if inputs.len() == 0 {
            return Ok(());
        }

        unsafe {
            let device = if inputs[0].r#type == PT_PEN {
                self.pen
            } else {
                self.touch
            };

            let success = InjectSyntheticPointerInput(device, inputs).as_bool();
            if success {
                Ok(())
            } else {
                Err(windows::core::Error::from_win32())
            }
        }
    }
}

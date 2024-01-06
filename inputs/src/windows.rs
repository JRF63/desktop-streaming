use windows::UI::Input::Preview::Injection::{self, InjectedInputVisualizationMode};

pub struct InputInjector {
    inner: Injection::InputInjector,
}

impl Drop for InputInjector {
    fn drop(&mut self) {
        let array: [fn(&Injection::InputInjector) -> Result<(), windows::core::Error>; 3] = [
            Injection::InputInjector::UninitializeGamepadInjection,
            Injection::InputInjector::UninitializePenInjection,
            Injection::InputInjector::UninitializeTouchInjection,
        ];

        for uninit in array {
            if let Err(e) = uninit(&self.inner) {
                tracing::error!("InputInjector error: {}", e);

                #[cfg(test)]
                panic!("InputInjector error: {}", e);
            }
        }
    }
}

impl InputInjector {
    pub fn new() -> Result<Self, windows::core::Error> {
        let inner = Injection::InputInjector::TryCreate()?;

        // TODO: Maybe don't initialize everything?
        let visualization_mode = InjectedInputVisualizationMode::None;
        inner.InitializeGamepadInjection()?;
        inner.InitializePenInjection(visualization_mode)?;
        inner.InitializeTouchInjection(visualization_mode)?;

        Ok(Self { inner })
    }
}

#[cfg(feature = "developer_mode_enabled")]
#[cfg(test)]
mod tests {
    //! Tests requires developer mode and admin priviledges. Outside of tests, the user of the
    //! library needs to have a [manifest][manifest_link].
    //!
    //! [manifest_link]: https://github.com/microsoft/windows-rs/blob/7219668f80a459b47097bc524af304073c69ec4b/crates/samples/windows/core_app/register.cmd

    use super::*;
    use approx::assert_relative_eq;
    use rusty_xinput::{XInputHandle, XInputState};
    use windows::{
        Gaming::Input::GamepadButtons, UI::Input::Preview::Injection::InjectedInputGamepadInfo,
    };

    #[test]
    fn input_injector_init_test() {
        InputInjector::new().unwrap();
    }

    #[test]
    fn input_injector_gamepad_test() {
        fn wait_for_xinput_to_register() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        let input_injector = InputInjector::new().unwrap();
        let xinput_handle = XInputHandle::load_default().unwrap();

        let gamepad_info = InjectedInputGamepadInfo::new().unwrap();

        wait_for_xinput_to_register();

        // Sticks
        let stick_range = [-1.0, 0.0, 1.0];
        for &x in &stick_range {
            for &y in &stick_range {
                gamepad_info.SetLeftThumbstickX(x).unwrap();
                gamepad_info.SetLeftThumbstickY(y).unwrap();
                gamepad_info.SetRightThumbstickX(y).unwrap();
                gamepad_info.SetRightThumbstickY(x).unwrap();

                input_injector
                    .inner
                    .InjectGamepadInput(&gamepad_info)
                    .unwrap();
                wait_for_xinput_to_register();

                let r = (x * x + y * y).sqrt();
                let (x, y) = {
                    if r != 0.0 {
                        ((x / r) as f32, (y / r) as f32)
                    } else {
                        (x as f32, y as f32)
                    }
                };

                let xinput_state = xinput_handle.get_state(0).unwrap();

                let (ls_x, ls_y) = xinput_state.left_stick_normalized();
                assert_relative_eq!(ls_x, x, epsilon = 1e-3);
                assert_relative_eq!(ls_y, y, epsilon = 1e-3);

                let (rs_x, rs_y) = xinput_state.right_stick_normalized();
                assert_relative_eq!(rs_x, y, epsilon = 1e-3);
                assert_relative_eq!(rs_y, x, epsilon = 1e-3);
            }
        }

        // Triggers
        let trigger_range = [0.0, 0.25, 0.5, 0.75, 1.0];
        for val in trigger_range {
            let left_value = val;
            let right_value = 1.0 - val;
            gamepad_info.SetLeftTrigger(left_value).unwrap();
            gamepad_info.SetRightTrigger(right_value).unwrap();

            input_injector
                .inner
                .InjectGamepadInput(&gamepad_info)
                .unwrap();
            wait_for_xinput_to_register();

            let xinput_state = xinput_handle.get_state(0).unwrap();

            let lt = xinput_state.left_trigger();
            let rt = xinput_state.right_trigger();

            let lt = (lt as f64) / 255.0;
            let rt = (rt as f64) / 255.0;

            assert_relative_eq!(lt, left_value, epsilon = 1e-2);
            assert_relative_eq!(rt, right_value, epsilon = 1e-2);
        }

        // Buttons
        // GamepadButtons::Paddle1 to GamepadButtons::Paddle4 are untestable
        let all_buttons: [(GamepadButtons, fn(&XInputState) -> bool); 14] = [
            (GamepadButtons::Menu, XInputState::start_button),
            (GamepadButtons::View, XInputState::select_button),
            (GamepadButtons::A, XInputState::south_button),
            (GamepadButtons::B, XInputState::east_button),
            (GamepadButtons::X, XInputState::west_button),
            (GamepadButtons::Y, XInputState::north_button),
            (GamepadButtons::DPadUp, XInputState::arrow_up),
            (GamepadButtons::DPadDown, XInputState::arrow_down),
            (GamepadButtons::DPadLeft, XInputState::arrow_left),
            (GamepadButtons::DPadRight, XInputState::arrow_right),
            (GamepadButtons::LeftShoulder, XInputState::left_shoulder),
            (GamepadButtons::RightShoulder, XInputState::right_shoulder),
            (
                GamepadButtons::LeftThumbstick,
                XInputState::left_thumb_button,
            ),
            (
                GamepadButtons::RightThumbstick,
                XInputState::right_thumb_button,
            ),
        ];
        for (button, state_getter) in all_buttons {
            gamepad_info.SetButtons(button).unwrap();

            input_injector
                .inner
                .InjectGamepadInput(&gamepad_info)
                .unwrap();
            wait_for_xinput_to_register();

            let xinput_state = xinput_handle.get_state(0).unwrap();
            assert!(state_getter(&xinput_state));
        }
    }
}

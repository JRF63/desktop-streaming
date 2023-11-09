use bitflags::bitflags;
use windows::UI::Input::Preview::Injection::{
    self, InjectedInputVisualizationMode,
};

pub struct InputInjector {
    inner: Injection::InputInjector,
    initialized_devices: InputDevices,
}

impl Drop for InputInjector {
    fn drop(&mut self) {
        let array: [(
            InputDevices,
            fn(&Injection::InputInjector) -> Result<(), windows::core::Error>,
        ); 3] = [
            (
                InputDevices::Gamepad,
                Injection::InputInjector::UninitializeGamepadInjection,
            ),
            (
                InputDevices::Pen,
                Injection::InputInjector::UninitializePenInjection,
            ),
            (
                InputDevices::Touch,
                Injection::InputInjector::UninitializeTouchInjection,
            ),
        ];

        for (device, uninit) in array {
            if self.initialized_devices.contains(device) {
                if let Err(e) = uninit(&self.inner) {
                    tracing::error!("InputInjector error: {}", e);
                }
            }
        }
    }
}

impl InputInjector {
    pub fn new() -> Result<Self, windows::core::Error> {
        let mut input_injector = Self {
            inner: Injection::InputInjector::TryCreate()?,
            initialized_devices: InputDevices::empty(),
        };

        // TODO: Maybe don't initialize everything?
        input_injector.initialize_gamepad()?;
        input_injector.initialize_pen()?;
        input_injector.initialize_touch()?;

        Ok(input_injector)
    }

    pub fn initialize_gamepad(&mut self) -> Result<(), windows::core::Error> {
        self.initialized_devices |= InputDevices::Gamepad;
        self.inner.InitializeGamepadInjection()
    }

    pub fn initialize_pen(&mut self) -> Result<(), windows::core::Error> {
        self.initialized_devices |= InputDevices::Pen;
        self.inner
            .InitializePenInjection(InjectedInputVisualizationMode::None)
    }

    pub fn initialize_touch(&mut self) -> Result<(), windows::core::Error> {
        self.initialized_devices |= InputDevices::Touch;
        self.inner
            .InitializeTouchInjection(InjectedInputVisualizationMode::None)
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct InputDevices: u8 {
        const Gamepad = 1;
        const Pen = 2;
        const Touch = 3;
    }
}

#[cfg(test)]
mod tests {
    //! Tests requires developer mode and admin priviledges

    use super::*;
    use tracing_subscriber::FmtSubscriber;

    fn init_tracing() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(tracing::Level::TRACE)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");
    }

    #[test]
    fn input_injector_init_test() {
        init_tracing();
        InputInjector::new().unwrap();
    }

    #[test]
    fn input_injection_test() -> windows::core::Result<()> {
        use std::time::{Duration, Instant};
        use windows::UI::Input::Preview::Injection::{InjectedInputGamepadInfo, InputInjector};

        let injector = InputInjector::TryCreate()?;
        injector.InitializeGamepadInjection()?;

        let neutral = InjectedInputGamepadInfo::new()?;
        let left_thumbstick = InjectedInputGamepadInfo::new()?;
        left_thumbstick.SetLeftThumbstickX(-1.0)?;

        let start = Instant::now();
        let mut moving = false;
        loop {
            moving = !moving;
            if moving {
                injector.InjectGamepadInput(&left_thumbstick)?;
            } else {
                injector.InjectGamepadInput(&neutral)?;
            }

            std::thread::sleep(Duration::from_millis(300));

            if start.elapsed() > Duration::from_secs(30) {
                break;
            }
        }

        Ok(())
    }
}


#[test]
fn input_injection_test() -> windows::core::Result<()> {
    use windows::UI::Input::Preview::Injection::InputInjector;

    let injector = InputInjector::TryCreate()?;

    Ok(())
}
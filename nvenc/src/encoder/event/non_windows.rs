use super::EventObjectTrait;

#[repr(transparent)]
pub struct EventObject(());

impl EventObjectTrait for EventObject {
    fn new() -> crate::Result<Self> {
        Ok(EventObject(()))
    }

    fn wait(&self) -> crate::Result<()> {
        Ok(())
    }

    fn as_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}
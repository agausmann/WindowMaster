use std::pin::Pin;

use crate::control::ControlBackend;

pub struct WindowsControlBackend {}

impl ControlBackend for WindowsControlBackend {
    type Error = windows::Error;

    fn start(
        self,
        _handle: crate::control::ControlHandle,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), Self::Error>>>> {
        Box::pin(async { todo!() })
    }
}

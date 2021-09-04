use std::{convert::Infallible, future::Future, pin::Pin, time::Duration};

use smol::Timer;
use windowmaster::{
    control::{ChannelInput, ControlBackend, ControlHandle, ControlInput},
    core::Core,
    thread_init,
    windows_backend::WindowsAudioBackend,
};

pub fn main() -> anyhow::Result<()> {
    env_logger::init();
    thread_init()?;

    Core::new(WindowsAudioBackend::new(), TestBackend)
        .run()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

struct TestBackend;

impl ControlBackend for TestBackend {
    type Error = Infallible;

    fn start(
        self,
        handle: ControlHandle,
    ) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>>>> {
        Box::pin(async move {
            Timer::after(Duration::from_secs(1)).await;
            handle
                .send(ControlInput::ChannelInput(0, ChannelInput::ToggleMuted))
                .await;
            Ok(())
        })
    }
}

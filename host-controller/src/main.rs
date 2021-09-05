use windowmaster::{
    backend::{hidapi::HidApiControlBackend, windows::WindowsAudioBackend},
    core::Core,
    thread_init,
};

pub fn main() -> anyhow::Result<()> {
    env_logger::init();
    thread_init()?;

    Core::new(WindowsAudioBackend::new(), HidApiControlBackend)
        .run()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

use windowmaster::{
    backend::{hidapi::HidApiControlBackend, windows::WindowsAudioBackend},
    core::Core,
};

pub fn main() -> anyhow::Result<()> {
    env_logger::init();

    Core::new(WindowsAudioBackend::new(), HidApiControlBackend)
        .run()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

mod audio;
mod hid;
mod manager;

use manager::Manager;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = Manager::new();
    audio::enumerate()?;
    hid::enumerate(&mut manager)?;
    manager.run();
    Ok(())
}

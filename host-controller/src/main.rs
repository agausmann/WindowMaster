#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

mod audio;
mod hid;
mod manager;

use manager::Manager;

// Copy-paste these paths to the `build!` macro in build.rs
use bindings::Windows::Win32::System::Com::CoInitializeEx;
// Additional bindings imports that don't map to a path in `build!`
use bindings::Windows::Win32::System::Com::COINIT_MULTITHREADED;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED)? };

    let mut manager = Manager::new();
    audio::enumerate()?;
    hid::enumerate(&mut manager)?;
    manager.run();
    Ok(())
}

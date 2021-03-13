#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

mod audio;
mod hid;

use std::error::Error;

pub fn main() -> Result<(), Box<dyn Error>> {
    //audio::enumerate()?;
    hid::poll()?;
    Ok(())
}

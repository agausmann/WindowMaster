#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

mod audio;

use std::error::Error;

pub fn main() -> Result<(), Box<dyn Error>> {
    audio::enumerate()?;
    Ok(())
}

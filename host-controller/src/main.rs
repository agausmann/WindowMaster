#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

mod audio;
mod hid;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    audio::enumerate()?;
    hid::enumerate()?;
    Ok(())
}

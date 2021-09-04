pub(crate) mod bindings {
    windows::include_bindings!();
}

pub mod audio;
pub mod control;
pub mod core;

pub mod windows_backend;

use crate::bindings::Windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

pub fn thread_init() -> windows::Result<()> {
    unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    windows::build! {
        Windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        },
    }
}

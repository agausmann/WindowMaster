fn main() {
    windows::build!(
        windows::win32::audio::IPropertyStore,
        windows::win32::automation::VARENUM,
        windows::win32::core_audio::{
            EDataFlow, IAudioSessionControl2, IAudioSessionEnumerator, IAudioSessionManager2,
            IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, ISimpleAudioVolume,
            MMDeviceEnumerator,
        },
        windows::win32::process_status::K32GetModuleBaseNameW,
        windows::win32::structured_storage::PROPVARIANT,
        windows::win32::system_services::{OpenProcess, BOOL, PROCESS_ACCESS_RIGHTS, PWSTR},
        windows::win32::windows_programming::CloseHandle,
        windows::win32::windows_properties_system::PROPERTYKEY,
    );
}

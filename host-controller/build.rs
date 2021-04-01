fn main() {
    windows::build!(
        // audio.rs
        Windows::Win32::Audio::IPropertyStore,
        Windows::Win32::Automation::VARENUM,
        Windows::Win32::CoreAudio::{
            EDataFlow, IAudioSessionControl2, IAudioSessionEnumerator, IAudioSessionManager2,
            IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, ISimpleAudioVolume,
            MMDeviceEnumerator,
        },
        Windows::Win32::ProcessStatus::K32GetModuleBaseNameW,
        Windows::Win32::StructuredStorage::PROPVARIANT,
        Windows::Win32::SystemServices::{OpenProcess, BOOL, PROCESS_ACCESS_RIGHTS, PWSTR},
        Windows::Win32::WindowsProgramming::CloseHandle,
        Windows::Win32::WindowsPropertiesSystem::PROPERTYKEY,

        // hid.rs
        Windows::Win32::DeviceAndDriverInstallation::{
            SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
            SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
        },
        Windows::Win32::FileSystem::{
            CreateFileW, ReadFile, WriteFile, FILE_ACCESS_FLAGS, FILE_CREATION_DISPOSITION,
            FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE,
        },
        Windows::Win32::Hid::{HidD_GetAttributes, HidD_GetHidGuid, HIDD_ATTRIBUTES},
        Windows::Win32::SystemServices::{HANDLE, PWSTR},
        Windows::Win32::WindowsAndMessaging::HWND,
        Windows::Win32::WindowsProgramming::CloseHandle,
    );
}

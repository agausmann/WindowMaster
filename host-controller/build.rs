fn main() {
    windows::build!(
        // main.rs
        Windows::Win32::System::Com::CoInitializeEx,

        // audio.rs
        Windows::Win32::Foundation::{CloseHandle, BOOL, PWSTR},
        Windows::Win32::Media::Audio::CoreAudio::{
            IAudioSessionControl2, IAudioSessionEnumerator, IAudioSessionManager2, IMMDevice,
            IMMDeviceCollection, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
        },
        Windows::Win32::Storage::StructuredStorage::PROPVARIANT,
        Windows::Win32::System::Com::CoCreateInstance,
        Windows::Win32::System::ProcessStatus::K32GetModuleBaseNameW,
        Windows::Win32::System::PropertiesSystem::{IPropertyStore, PROPERTYKEY},
        Windows::Win32::System::Threading::OpenProcess,

        Windows::Win32::System::OleAutomation::VARENUM,

        // hid.rs
        Windows::Win32::Devices::DeviceAndDriverInstallation::{
            SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
            SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
        },
        Windows::Win32::Devices::HumanInterfaceDevice::{
            HidD_GetAttributes, HidD_GetHidGuid, HIDD_ATTRIBUTES,
        },
        Windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, PWSTR},
        Windows::Win32::Storage::FileSystem::{CreateFileW, ReadFile, WriteFile},
    );
}

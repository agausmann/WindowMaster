fn main() {
    windows::build!(
        // audio.rs
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

        // hid.rs
    windows::win32::device_and_driver_installation::{
        SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
        SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    },
    windows::win32::file_system::{
        CreateFileW, FILE_ACCESS_FLAGS, FILE_CREATE_FLAGS, FILE_FLAGS_AND_ATTRIBUTES,
        FILE_SHARE_FLAGS,
    },
    windows::win32::hid::{
        HidD_GetAttributes, HidD_GetHidGuid, HidD_GetManufacturerString, HidD_GetProductString,
        HIDD_ATTRIBUTES,
    },
    windows::win32::system_services::{HANDLE, PWSTR},
    windows::win32::windows_and_messaging::HWND,
    windows::win32::windows_programming::CloseHandle,
    );
}

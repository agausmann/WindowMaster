fn main() {
    // Does not depend on any other files.
    println!("cargo:rerun-if-changed=build.rs");

    windows::build! {
        Windows::Win32::{
            Media::Audio::CoreAudio::{
                DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED, DEVICE_STATE_NOTPRESENT,
                DEVICE_STATE_UNPLUGGED, DEVICE_STATEMASK_ALL, IMMDeviceEnumerator,
                IMMNotificationClient, MMDeviceEnumerator, IMMDevice, IAudioSessionManager2,
                IAudioEndpointVolume, IAudioEndpointVolumeCallback, IMMDeviceCollection,
                IAudioSessionNotification, IAudioSessionControl, IAudioSessionEvents,
                AudioSessionState, AudioSessionDisconnectReason, IAudioSessionControl2,
                IAudioSessionEnumerator, ISimpleAudioVolume
            },
            Storage::StructuredStorage::STGM_READ,
            System::{
                Com::{CoInitializeEx, CoCreateInstance},
                OleAutomation::VARENUM,
                PropertiesSystem::IPropertyStore,
                SystemServices::DEVPKEY_Device_FriendlyName,
            },
        },
    }
}

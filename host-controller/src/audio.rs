// Copy-paste these paths to the `build!` macro in build.rs
use crate::bindings::{
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
};
// Required to bring some bindings in scope (also copy to build.rs)
#[allow(unused_imports)]
use crate::bindings::Windows::Win32::System::OleAutomation::VARENUM;
// Additional bindings imports that don't map to a path in `build!`
use crate::bindings::{
    Windows::Win32::Media::Audio::CoreAudio::eAll,
    Windows::Win32::Storage::StructuredStorage::{
        PROPVARIANT_0_0_0_abi, PROPVARIANT_0_0_abi, PROPVARIANT_0,
    },
    Windows::Win32::System::Com::CLSCTX_ALL,
    Windows::Win32::System::OleAutomation::{VT_EMPTY, VT_LPWSTR},
    Windows::Win32::System::Threading::{PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
};

use widestring::U16CStr;
use windows::{Guid, Interface};

// Constants that aren't yet provided in the Windows bindings
const DEVICE_STATE_ACTIVE: u32 = 0x1;
const STGM_READ: u32 = 0x0;
#[allow(non_upper_case_globals)]
const PKEY_Device_FriendlyName: PROPERTYKEY = PROPERTYKEY {
    // {A45C254E-DF1C-4EFD-8020-67D146A850E0} 14
    fmtid: Guid::from_values(
        0xA45C254E,
        0xDF1C,
        0x4EFD,
        [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0],
    ),
    pid: 14,
};

pub fn enumerate() -> Result<(), Box<dyn std::error::Error>> {
    let device_enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let devices: IMMDeviceCollection =
        unsafe { device_enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)? };
    let device_count = unsafe { devices.GetCount()? };
    for device_index in 0..device_count {
        let device: IMMDevice = unsafe { devices.Item(device_index)? };

        let property_store: IPropertyStore = unsafe { device.OpenPropertyStore(STGM_READ)? };

        let name_prop = unsafe { property_store.GetValue(&PKEY_Device_FriendlyName)?.into() };
        let name = match name_prop {
            Property::Pwstr(pwstr) => unsafe { pwstr_to_string(&pwstr) },
            _ => unreachable!(),
        };
        println!("{}: {}", device_index, name);

        let mut session_manager = None;
        let session_manager: IAudioSessionManager2 = unsafe {
            device
                .Activate(
                    &IAudioSessionManager2::IID,
                    0,
                    std::ptr::null_mut(),
                    &mut session_manager as *mut _ as _,
                )
                .map(|_| session_manager.unwrap())?
        };

        let session_enumerator: IAudioSessionEnumerator =
            unsafe { session_manager.GetSessionEnumerator()? };

        let session_count = unsafe { session_enumerator.GetCount()? };

        for session_index in 0..session_count {
            let session = unsafe {
                session_enumerator
                    .GetSession(session_index)?
                    .cast::<IAudioSessionControl2>()?
            };
            let volume_control = match session.cast::<ISimpleAudioVolume>() {
                Ok(x) => x,
                _ => {
                    println!("(skipped)");
                    continue;
                }
            };
            // unsafe {
            //     volume_control
            //         .SetMute(BOOL::from(false), std::ptr::null_mut())
            //         .unwrap();
            // }
            let process_id = unsafe { session.GetProcessId()? };

            let mut name = String::new();
            if unsafe { session.IsSystemSoundsSession().is_ok() } {
                name = "System Sounds".into();
            }
            if name.is_empty() {
                let name_ptr = unsafe { session.GetDisplayName()? };
                if !name_ptr.is_null() {
                    name = unsafe { pwstr_to_string(&name_ptr) };
                }
            }
            if name.is_empty() {
                let process_handle = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                        BOOL::from(false),
                        process_id,
                    )
                };
                if process_handle.is_null() || process_handle.is_invalid() {
                    panic!("error: cannot open process"); //TODO make recoverable
                }
                const MAX_LEN: u32 = 100;
                let mut buffer = vec![0u16; MAX_LEN as _];
                unsafe {
                    K32GetModuleBaseNameW(process_handle, None, PWSTR(buffer.as_mut_ptr()), MAX_LEN)
                };
                unsafe { CloseHandle(process_handle).ok()? };

                name = U16CStr::from_slice_with_nul(&buffer)
                    .unwrap()
                    .to_string_lossy();
            }
            if let Some(stripped) = name.strip_suffix(".exe") {
                name = stripped.to_string();
            }
            //println!("    {}: {} {}", session_index, process_id, name);
            println!("    {}: {}", session_index, name);
        }
    }

    Ok(())
}

unsafe fn pwstr_to_string(pwstr: &PWSTR) -> String {
    U16CStr::from_ptr_str(pwstr.0).to_string_lossy()
}

enum Property {
    Empty,
    Pwstr(PWSTR),
}

impl From<PROPVARIANT> for Property {
    fn from(variant: PROPVARIANT) -> Self {
        unsafe {
            match variant {
                PROPVARIANT {
                    Anonymous:
                        PROPVARIANT_0 {
                            Anonymous: PROPVARIANT_0_0_abi { vt, .. },
                        },
                } if vt == VT_EMPTY.0 as _ => Property::Empty,
                PROPVARIANT {
                    Anonymous:
                        PROPVARIANT_0 {
                            Anonymous:
                                PROPVARIANT_0_0_abi {
                                    vt,
                                    Anonymous: PROPVARIANT_0_0_0_abi { pwszVal },
                                    ..
                                },
                        },
                } if vt == VT_LPWSTR.0 as _ => Property::Pwstr(pwszVal),
                _ => unimplemented!(),
            }
        }
    }
}

impl From<Property> for PROPVARIANT {
    fn from(prop: Property) -> Self {
        match prop {
            Property::Empty => PROPVARIANT {
                Anonymous: PROPVARIANT_0 {
                    Anonymous: PROPVARIANT_0_0_abi {
                        vt: VT_EMPTY.0 as _,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: PROPVARIANT_0_0_0_abi { bVal: 0 },
                    },
                },
            },
            _ => unimplemented!(),
        }
    }
}

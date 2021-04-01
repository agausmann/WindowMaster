// Copy-paste these paths to the `build!` macro in build.rs
use crate::bindings::{
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
};
// Additional bindings imports that don't map to a path in `build!`
use crate::bindings::Windows::Win32::StructuredStorage::{
    PROPVARIANT_0_0_0_abi, PROPVARIANT_0_0_abi, PROPVARIANT_0,
};

use widestring::U16CStr;
use windows::{ErrorCode, Guid, Interface};

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
    windows::initialize_mta()?;
    let device_enumerator: IMMDeviceEnumerator = windows::create_instance(&MMDeviceEnumerator)?;
    let mut devices = None;
    let devices: IMMDeviceCollection = unsafe {
        device_enumerator
            .EnumAudioEndpoints(EDataFlow::eAll, DEVICE_STATE_ACTIVE, &mut devices)
            .and_some(devices)?
    };
    let mut device_count = Default::default();
    let device_count = unsafe {
        devices
            .GetCount(&mut device_count)
            .and_then(|| device_count)?
    };
    for device_index in 0..device_count {
        let mut device = None;
        let device: IMMDevice =
            unsafe { devices.Item(device_index, &mut device).and_some(device)? };

        let mut property_store = None;
        let property_store: IPropertyStore = unsafe {
            device
                .OpenPropertyStore(STGM_READ, &mut property_store)
                .and_some(property_store)?
        };

        let mut name_variant = Property::Empty.into();
        let name_prop = unsafe {
            property_store
                .GetValue(&PKEY_Device_FriendlyName, &mut name_variant)
                .and_then(|| name_variant.into())?
        };
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
                .and_some(session_manager)?
        };

        let mut session_enumerator = None;
        let session_enumerator: IAudioSessionEnumerator = unsafe {
            session_manager
                .GetSessionEnumerator(&mut session_enumerator)
                .and_some(session_enumerator)?
        };

        let mut session_count = 0;
        let session_count = unsafe {
            session_enumerator
                .GetCount(&mut session_count)
                .and_then(|| session_count)?
        };

        for session_index in 0..session_count {
            let mut session = None;
            let session = unsafe {
                session_enumerator
                    .GetSession(session_index, &mut session)
                    .and_some(session)?
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
            let mut process_id = Default::default();
            let process_id = unsafe {
                session
                    .GetProcessId(&mut process_id)
                    .and_then(|| process_id)?
            };

            let mut name = String::new();
            if unsafe { session.IsSystemSoundsSession() } == ErrorCode::S_OK {
                name = "System Sounds".into();
            }
            if name.is_empty() {
                let mut name_ptr = Default::default();
                let name_ptr = unsafe {
                    session
                        .GetDisplayName(&mut name_ptr)
                        .and_then(|| name_ptr)?
                };
                if !name_ptr.0.is_null() {
                    name = unsafe { pwstr_to_string(&name_ptr) };
                }
            }
            if name.is_empty() {
                let process_handle = unsafe {
                    OpenProcess(
                        PROCESS_ACCESS_RIGHTS::PROCESS_QUERY_INFORMATION
                            | PROCESS_ACCESS_RIGHTS::PROCESS_VM_READ,
                        BOOL::from(false),
                        process_id,
                    )
                };
                if process_handle.0 == 0 {
                    Err(windows::Error::from(ErrorCode::from_thread()))?;
                }
                const MAX_LEN: u32 = 100;
                let mut buffer = vec![0u16; MAX_LEN as _];
                unsafe {
                    K32GetModuleBaseNameW(process_handle, 0, PWSTR(buffer.as_mut_ptr()), MAX_LEN)
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
                } if vt == VARENUM::VT_EMPTY.0 as _ => Property::Empty,
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
                } if vt == VARENUM::VT_LPWSTR.0 as _ => Property::Pwstr(pwszVal),
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
                        vt: VARENUM::VT_EMPTY.0 as _,
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

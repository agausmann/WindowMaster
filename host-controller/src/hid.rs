// Copy to build.rs
use crate::bindings::{
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
};

use std::error::Error;
use widestring::U16CStr;

const DIGCF_DEVICEINTERFACE: u32 = 0x10;
const DIGCF_PRESENT: u32 = 0x2;
const ERROR_NO_MORE_ITEMS: u32 = 0x80070103;

pub fn poll() -> Result<(), Box<dyn Error>> {
    let mut hid_guid = Default::default();
    unsafe { HidD_GetHidGuid(&mut hid_guid) };

    let device_set = unsafe {
        SetupDiGetClassDevsW(
            &hid_guid,
            PWSTR::default(),
            HWND::default(),
            DIGCF_DEVICEINTERFACE | DIGCF_PRESENT,
        )
    };
    for index in 0.. {
        let mut device_interface: SP_DEVICE_INTERFACE_DATA = Default::default();
        device_interface.cb_size = std::mem::size_of_val(&device_interface) as _;
        let success = unsafe {
            SetupDiEnumDeviceInterfaces(
                device_set,
                std::ptr::null_mut(),
                &hid_guid,
                index,
                &mut device_interface,
            )
        };
        match success.ok() {
            Ok(()) => {}
            Err(error) if error.code().0 == ERROR_NO_MORE_ITEMS => break,
            Err(error) => Err(error)?,
        }

        let mut device_interface_details: SP_DEVICE_INTERFACE_DETAIL_DATA_W_CUSTOM =
            Default::default();
        device_interface_details.cb_size =
            std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as _;
        unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                device_set,
                &mut device_interface,
                &mut device_interface_details as *mut _ as _,
                std::mem::size_of_val(&device_interface_details) as _,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
            .ok()?
        };

        let handle = unsafe {
            CreateFileW(
                PWSTR(device_interface_details.device_path.as_mut_ptr()),
                FILE_ACCESS_FLAGS::FILE_GENERIC_READ | FILE_ACCESS_FLAGS::FILE_GENERIC_WRITE,
                FILE_SHARE_FLAGS::FILE_SHARE_NONE,
                std::ptr::null_mut(),
                FILE_CREATE_FLAGS::OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES::FILE_ATTRIBUTE_NORMAL,
                HANDLE::default(),
            )
        };
        if handle == HANDLE(-1) {
            // No access
            continue;
        }
        let mut attributes: HIDD_ATTRIBUTES = Default::default();
        const MAX_LEN: u32 = 100;
        let mut manu_string = vec![0u16; MAX_LEN as _];
        let mut product_string = vec![0u16; MAX_LEN as _];
        unsafe { HidD_GetAttributes(handle, &mut attributes) };
        unsafe { HidD_GetManufacturerString(handle, manu_string.as_mut_ptr() as _, MAX_LEN) };
        unsafe { HidD_GetProductString(handle, product_string.as_mut_ptr() as _, MAX_LEN) };
        unsafe { CloseHandle(handle).ok()? };

        let manu_string = U16CStr::from_slice_with_nul(&manu_string)
            .unwrap()
            .to_string_lossy();
        let product_string = U16CStr::from_slice_with_nul(&product_string)
            .unwrap()
            .to_string_lossy();
        println!(
            "{:04x}:{:04x} {} {}",
            attributes.vendor_id, attributes.product_id, manu_string, product_string
        );
    }

    Ok(())
}

// A sized version of SP_DEVICE_INTERFACE_DETAIL_DATA_W, can store a path of up to 1k chars.
#[repr(C)]
struct SP_DEVICE_INTERFACE_DETAIL_DATA_W_CUSTOM {
    cb_size: u32,
    device_path: [u16; 1000],
}

impl Default for SP_DEVICE_INTERFACE_DETAIL_DATA_W_CUSTOM {
    fn default() -> Self {
        Self {
            cb_size: Default::default(),
            device_path: [Default::default(); 1000],
        }
    }
}

// Copy to build.rs
use crate::bindings::{
    Windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
        SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    },
    Windows::Win32::Devices::HumanInterfaceDevice::{
        HidD_GetAttributes, HidD_GetHidGuid, HIDD_ATTRIBUTES,
    },
    Windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, PWSTR},
    Windows::Win32::Storage::FileSystem::{CreateFileW, ReadFile, WriteFile},
};
// Bindings imports that don't map to a path in `build!`
use crate::bindings::Windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};

use crate::manager::{Handle, Input, Manager, Update};
use lazy_static::lazy_static;
use std::cell::Cell;
use std::convert::{TryFrom, TryInto};
use std::ffi::c_void;
use std::marker::PhantomData;
use std::time::Instant;

const DIGCF_DEVICEINTERFACE: u32 = 0x10;
const DIGCF_PRESENT: u32 = 0x2;
const ERROR_NO_MORE_ITEMS: u32 = 0x80070103;

lazy_static! {
    // Used to time blinking indicators.
    static ref EPOCH: Instant = Instant::now();
}

#[derive(Debug)]
struct Device {
    handle: HANDLE,
    device_type: DeviceType,
    state: DeviceState,

    // Just as a precaution, prevent Device from being used by multiple threads
    // simultaneously.
    _not_sync: PhantomData<Cell<()>>,
}

impl Device {
    /// # Safety
    /// This function takes ownership of the passed-in handle, meaning it should
    /// have exclusive access to that handle object, and the handle will be
    /// closed when the returned `Device` object is dropped. If the function
    /// returns `None`, ownership is relinquished, and the caller is responsible
    /// for closing the handle.
    unsafe fn from_handle(handle: HANDLE, manager: &mut Manager) -> Option<Self> {
        let device_type = DeviceType::detect(handle)?;
        let state = DeviceState {
            channels: std::iter::repeat_with(|| ChannelState::new(manager.register_channel()))
                .take(device_type.num_channels())
                .collect(),
        };
        Some(Self {
            handle,
            device_type,
            state,
            _not_sync: PhantomData,
        })
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            self.poll()?;
            for channel in self.state.channels.iter_mut() {
                if channel.pressed.is_none() && channel.pressed_changed && !channel.held {
                    channel.handle.input(Input::ToggleMute);
                }
                if let Some(pressed) = channel.pressed {
                    if pressed.elapsed().as_millis() >= 500 && !channel.held {
                        channel.handle.input(Input::OpenMenu);
                        channel.held = true;
                    }
                }
                if channel.pressed.is_none() {
                    channel.held = false;
                }
                let steps = channel.take_steps();
                if steps != 0 {
                    if channel.menu_open {
                        channel.handle.input(Input::MenuStep(steps));
                    } else {
                        channel.handle.input(Input::VolumeStep(steps));
                    }
                }
                while let Some(update) = channel.handle.poll_updates() {
                    match update {
                        Update::Mute(muted) => {
                            channel.muted = muted;
                        }
                        Update::OpenMenu(options, index) => {
                            channel.menu_open = true;
                            for (i, opt) in options.iter().enumerate() {
                                println!("{}. {}", i, opt);
                            }
                            println!("{}", index);
                        }
                        Update::CloseMenu => {
                            channel.menu_open = false;
                        }
                        // Not used by this device
                        Update::Volume(_) => {}
                        Update::MenuIndex(index) => {
                            println!("{}", index);
                        }
                    }
                }
            }
        }
    }

    fn poll(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut output_report: [u8; 2] = [0x0, 0x0];
        for (i, channel) in self.state.channels.iter().enumerate() {
            // Set the indicator's base state according to the channel's mute
            // state, and incorporate a momentary blink if the menu is open.
            // Boolean to determine blink state - is true for 200ms per 1000.
            let blink = EPOCH.elapsed().as_millis() % 1000 <= 200;
            let indicator_on = channel.muted ^ (channel.menu_open && blink);
            if indicator_on {
                output_report[1] |= 1 << i;
            }
        }
        let mut bytes_written: u32 = 0;
        unsafe {
            WriteFile(
                self.handle,
                output_report.as_ptr() as *const c_void,
                u32::try_from(output_report.len()).unwrap(),
                &mut bytes_written,
                std::ptr::null_mut(),
            )
            .ok()?
        };

        let mut input_report: [u8; 8] = [0; 8];
        let mut bytes_read: u32 = 0;
        unsafe {
            ReadFile(
                self.handle,
                input_report.as_mut_ptr() as *mut c_void,
                u32::try_from(input_report.len()).unwrap(),
                &mut bytes_read,
                std::ptr::null_mut(),
            )
            .ok()?
        };
        let num_channels = self.state.channels.len();
        for (i, channel) in self.state.channels.iter_mut().enumerate() {
            channel.steps += input_report[1 + i] as i8 as i32;
            let new_pressed = (input_report[1 + num_channels] & (1 << i)) != 0;
            channel.pressed_changed = new_pressed ^ channel.pressed.is_some();
            if channel.pressed_changed {
                if new_pressed {
                    channel.pressed = Some(Instant::now());
                } else {
                    channel.pressed = None;
                }
            }
        }

        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

#[derive(Debug)]
enum DeviceType {
    WindowMasterRev1,
}

impl DeviceType {
    fn detect(handle: HANDLE) -> Option<Self> {
        let mut attributes: HIDD_ATTRIBUTES = Default::default();
        attributes.Size = std::mem::size_of_val(&attributes).try_into().unwrap();
        unsafe { HidD_GetAttributes(handle, &mut attributes) };

        match (
            attributes.VendorID,
            attributes.ProductID,
            attributes.VersionNumber,
        ) {
            (0x1209, 0x4573, 0x0010) => Some(Self::WindowMasterRev1),
            _ => None,
        }
    }

    fn num_channels(&self) -> usize {
        match self {
            Self::WindowMasterRev1 => 6,
        }
    }
}

#[derive(Debug)]
struct DeviceState {
    channels: Vec<ChannelState>,
}

#[derive(Debug)]
struct ChannelState {
    handle: Handle,
    pressed: Option<Instant>,
    pressed_changed: bool,
    held: bool,
    steps: i32,
    menu_open: bool,
    muted: bool,
}

impl ChannelState {
    fn take_steps(&mut self) -> i32 {
        std::mem::replace(&mut self.steps, 0)
    }

    fn new(handle: Handle) -> Self {
        Self {
            handle,
            pressed: None,
            pressed_changed: false,
            held: false,
            steps: 0,
            menu_open: false,
            muted: false,
        }
    }
}

pub fn enumerate(manager: &mut Manager) -> Result<(), Box<dyn std::error::Error>> {
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

    let mut devices = Vec::new();

    for index in 0.. {
        let mut device_interface: SP_DEVICE_INTERFACE_DATA = Default::default();
        device_interface.cbSize = std::mem::size_of_val(&device_interface) as _;
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
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                HANDLE::default(),
            )
        };
        if handle == HANDLE(-1) {
            // No access
            continue;
        }
        //SAFETY: `handle` binding is dropped after this iteration and is not
        // held anywhere else, so the File successfully has ownership from here
        // on. If None is returned, the handle is immediately closed.
        if let Some(device) = unsafe { Device::from_handle(handle, manager) } {
            println!("{:?}", device);
            devices.push(device);
        } else {
            unsafe { CloseHandle(handle).ok()? };
        }
    }

    for mut device in devices {
        std::thread::spawn(move || device.run().unwrap());
    }

    Ok(())
}

// A sized version of SP_DEVICE_INTERFACE_DETAIL_DATA_W, can store a path of up
// to 1k chars.
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

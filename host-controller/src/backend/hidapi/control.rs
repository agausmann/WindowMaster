use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

use bimap::BiHashMap;
use bytemuck::Zeroable;
use hidapi::{HidApi, HidDevice, HidError};
use once_cell::sync::Lazy;
use smol::channel::TryRecvError;

use crate::control::{
    ChannelInput, ChannelOutput, ControlBackend, ControlHandle, ControlInput, ControlOutput,
    DeviceId, DeviceInfo, DeviceInfoBuilder,
};

pub struct HidApiControlBackend;

impl ControlBackend for HidApiControlBackend {
    type Error = HidError;

    fn start(
        self,
        handle: ControlHandle,
    ) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>>>> {
        Box::pin(smol::unblock(|| {
            let mut runtime = Runtime {
                handle,
                devices: HashMap::new(),
                device_keys: BiHashMap::new(),
                hidapi: HidApi::new()?,
            };
            runtime.run()
        }))
    }
}

struct Runtime {
    handle: ControlHandle,
    devices: HashMap<DeviceId, Device>,
    device_keys: BiHashMap<DeviceId, DeviceKey>,
    hidapi: HidApi,
}

impl Runtime {
    fn run(&mut self) -> Result<(), HidError> {
        let mut refresh_timer = Timer::new(Duration::from_millis(1000));
        'main: loop {
            if refresh_timer.poll() {
                // Get new list of present devices
                self.hidapi.refresh_devices()?;
                let infos: HashMap<DeviceKey, &hidapi::DeviceInfo> = self
                    .hidapi
                    .device_list()
                    .map(|info| (DeviceKey::new(info), info))
                    .collect();

                // Handle devices that are no longer present.
                let mut to_remove = Vec::new();
                for (device_id, device_key) in self.device_keys.iter() {
                    if !infos.contains_key(device_key) {
                        to_remove.push(*device_id);
                    }
                }
                for device_id in to_remove {
                    self.device_keys.remove_by_left(&device_id);
                    self.devices.remove(&device_id);
                    self.handle
                        .blocking_send(ControlInput::DeviceRemoved(device_id));
                }

                // Handle devices that just became present.
                for (device_key, ll_info) in infos {
                    if self.device_keys.contains_right(&device_key) {
                        continue;
                    }
                    let device = match Device::detect(&self.hidapi, ll_info)? {
                        Some(x) => x,
                        None => continue,
                    };
                    let device_id = device.id();
                    let device_info = device.info();
                    self.device_keys
                        .insert_no_overwrite(device_id, device_key)
                        .expect("device key conflict");
                    self.devices.insert(device_id, device);
                    self.handle
                        .blocking_send(ControlInput::DeviceAdded(device_id, device_info));
                }
            }

            loop {
                match self.handle.try_recv() {
                    Ok(control_output) => {
                        log::debug!("incoming {:?}", control_output);
                        match control_output {
                            ControlOutput::ChannelOutput(
                                device_id,
                                channel_index,
                                channel_output,
                            ) => {
                                if let Some(device) = self.devices.get_mut(&device_id) {
                                    device.channel_output(channel_index, channel_output);
                                } else {
                                    log::warn!("received event for unknown device {:?}", device_id);
                                }
                            }
                        }
                    }
                    Err(TryRecvError::Closed) => {
                        log::debug!("closing");
                        break 'main;
                    }
                    Err(TryRecvError::Empty) => {
                        break;
                    }
                }
            }

            for device in self.devices.values_mut() {
                device.poll(&self.handle).ok();
            }

            // Prevent busy loop
            std::thread::yield_now();
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DeviceKey {
    vendor_id: u16,
    product_id: u16,
    release_number: u16,
    serial_number: Vec<u8>,
}

impl DeviceKey {
    fn new(info: &hidapi::DeviceInfo) -> Self {
        Self {
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            release_number: info.release_number(),
            serial_number: info
                .serial_number_raw()
                .map(|wchars| bytemuck::cast_slice(wchars).to_vec())
                .unwrap_or_else(|| {
                    // If the device does not have a serial number, fall back to the path.
                    // This isn't perfect because it is assigned by the OS and may change,
                    // but it's better than nothing.
                    info.path().to_bytes().to_vec()
                }),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DeviceModel {
    Rev1,
}

impl DeviceModel {
    fn detect(info: &hidapi::DeviceInfo) -> Option<Self> {
        match (info.vendor_id(), info.product_id()) {
            (0x1209, 0x4573) => Some(Self::Rev1),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Rev1 => "WindowMaster Rev1",
        }
    }

    fn num_channels(&self) -> usize {
        match self {
            Self::Rev1 => rev1::NUM_CHANNELS,
        }
    }

    fn device_info(&self) -> DeviceInfo {
        DeviceInfoBuilder::new(self.name().to_string(), self.num_channels()).build()
    }
}

struct Device {
    model: DeviceModel,
    state: DeviceState,
    ll_device: HidDevice,
    device_id: DeviceId,
}

impl Device {
    fn detect(hidapi: &HidApi, info: &hidapi::DeviceInfo) -> Result<Option<Self>, HidError> {
        let model = match DeviceModel::detect(info) {
            Some(x) => x,
            None => return Ok(None),
        };
        let state = DeviceState::new(model);
        let ll_device = info.open_device(hidapi)?;
        ll_device.set_blocking_mode(false)?;

        Ok(Some(Self {
            model,
            state,
            ll_device,
            device_id: DeviceId::new(),
        }))
    }

    fn id(&self) -> DeviceId {
        self.device_id
    }

    fn info(&self) -> DeviceInfo {
        self.model.device_info()
    }

    fn channel_output(&mut self, index: usize, channel_output: ChannelOutput) {
        match &mut self.state {
            DeviceState::Rev1(state) => {
                let channel = &mut state.channels[index];
                match channel_output {
                    ChannelOutput::StateChanged(state) => {
                        channel.state = state;
                    }
                    ChannelOutput::MenuOpened => {
                        channel.menu_open = true;
                    }
                    ChannelOutput::MenuClosed => {
                        channel.menu_open = false;
                    }
                }
            }
        }
    }

    fn poll(&mut self, handle: &ControlHandle) -> Result<(), HidError> {
        match &mut self.state {
            DeviceState::Rev1(state) => {
                // While there is data to read:
                let mut should_output = false;
                loop {
                    let mut input = rev1::Input::zeroed();
                    let num_read = self.ll_device.read(bytemuck::bytes_of_mut(&mut input))?;
                    if num_read == 0 {
                        // No more data available
                        break;
                    }
                    should_output = true;
                    assert_eq!(num_read, std::mem::size_of_val(&input));

                    let now = Instant::now();
                    for index in 0..rev1::NUM_CHANNELS {
                        let channel = &mut state.channels[index];

                        let pressed = input.buttons & (1 << index) != 0;
                        let steps = input.encoders[index];

                        if steps != 0 {
                            if channel.menu_open {
                                if steps > 0 {
                                    for _ in 0..steps {
                                        handle.blocking_send(ControlInput::ChannelInput(
                                            self.device_id,
                                            index,
                                            ChannelInput::MenuNext,
                                        ));
                                    }
                                } else {
                                    for _ in steps..0 {
                                        handle.blocking_send(ControlInput::ChannelInput(
                                            self.device_id,
                                            index,
                                            ChannelInput::MenuPrevious,
                                        ));
                                    }
                                }
                            } else {
                                handle.blocking_send(ControlInput::ChannelInput(
                                    self.device_id,
                                    index,
                                    ChannelInput::StepVolume(steps.into()),
                                ));
                            }
                        }

                        if pressed {
                            if !channel.pressed {
                                channel.long_press_timeout = Some(now + LONG_PRESS_DURATION);
                            }
                            if let Some(timeout) = channel.long_press_timeout {
                                if now >= timeout {
                                    let input = if channel.menu_open {
                                        ChannelInput::CloseMenu
                                    } else {
                                        ChannelInput::OpenMenu
                                    };
                                    handle.blocking_send(ControlInput::ChannelInput(
                                        self.device_id,
                                        index,
                                        input,
                                    ));
                                    channel.long_press_timeout = None;
                                    channel.long_pressed = true;
                                }
                            }
                        } else {
                            if channel.pressed && !channel.long_pressed {
                                let input = if channel.menu_open {
                                    ChannelInput::MenuSelect
                                } else {
                                    ChannelInput::ToggleMuted
                                };
                                handle.blocking_send(ControlInput::ChannelInput(
                                    self.device_id,
                                    index,
                                    input,
                                ));
                            }
                            channel.pressed = false;
                            channel.long_pressed = false;
                            channel.long_press_timeout = None;
                        }
                        channel.pressed = pressed;
                    }
                }
                if should_output {
                    let mut output = rev1::Output::zeroed();
                    let now = Instant::now();
                    let blink_phase = now
                        .saturating_duration_since(*MENU_BLINK_TIMER)
                        .as_secs_f32()
                        % MENU_BLINK_PERIOD.as_secs_f32()
                        < MENU_BLINK_DURATION.as_secs_f32();
                    for index in 0..rev1::NUM_CHANNELS {
                        let channel = &mut state.channels[index];
                        // Handle output
                        if channel.state.muted ^ (channel.menu_open && blink_phase) {
                            output.leds |= 1 << index;
                        }
                    }

                    let num_written = self.ll_device.write(bytemuck::bytes_of(&output))?;
                    assert_eq!(num_written, std::mem::size_of_val(&output));
                }
            }
        }
        Ok(())
    }
}

enum DeviceState {
    Rev1(rev1::DeviceState),
}

impl DeviceState {
    fn new(model: DeviceModel) -> Self {
        match model {
            DeviceModel::Rev1 => Self::Rev1(rev1::DeviceState::new()),
        }
    }
}

const LONG_PRESS_DURATION: Duration = Duration::from_millis(500);
const MENU_BLINK_PERIOD: Duration = Duration::from_millis(1000);
const MENU_BLINK_DURATION: Duration = Duration::from_millis(250);
static MENU_BLINK_TIMER: Lazy<Instant> = Lazy::new(|| Instant::now());

mod rev1 {
    use std::time::Instant;

    use bytemuck::{Pod, Zeroable};

    use crate::audio::StreamState;

    pub(crate) const NUM_CHANNELS: usize = 6;

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C)]
    pub(crate) struct Input {
        pub(crate) encoders: [i8; NUM_CHANNELS],
        pub(crate) buttons: u8,
    }

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C)]
    pub(crate) struct Output {
        pub(crate) report_id: u8,
        pub(crate) leds: u8,
    }

    pub(crate) struct DeviceState {
        pub(crate) channels: [ChannelState; NUM_CHANNELS],
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ChannelState {
        pub(crate) pressed: bool,
        pub(crate) long_pressed: bool,
        pub(crate) long_press_timeout: Option<Instant>,
        pub(crate) menu_open: bool,
        pub(crate) state: StreamState,
    }

    impl DeviceState {
        pub(crate) fn new() -> Self {
            Self {
                channels: [ChannelState {
                    pressed: false,
                    long_pressed: false,
                    long_press_timeout: None,
                    menu_open: false,
                    state: Default::default(),
                }; NUM_CHANNELS],
            }
        }
    }
}

struct Timer {
    period: Duration,
    next_timeout: Instant,
}

impl Timer {
    fn new(period: Duration) -> Self {
        Self {
            period,
            next_timeout: Instant::now(),
        }
    }

    fn poll(&mut self) -> bool {
        let now = Instant::now();
        if now > self.next_timeout {
            self.next_timeout += self.period;
            true
        } else {
            false
        }
    }
}

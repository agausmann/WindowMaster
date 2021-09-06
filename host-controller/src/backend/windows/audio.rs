use crate::audio::{
    AudioBackend, AudioControl, AudioEvent, AudioHandle, StreamControl, StreamEvent, StreamId,
};
use crate::bindings::Windows::Win32::{
    Foundation::PWSTR,
    Media::Audio::CoreAudio::{
        eConsole, eRender, EDataFlow, ERole, IAudioEndpointVolume, IAudioEndpointVolumeCallback,
        IMMDevice, IMMDeviceEnumerator, IMMNotificationClient, MMDeviceEnumerator,
        AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED,
        DEVICE_STATE_NOTPRESENT, DEVICE_STATE_UNPLUGGED,
    },
    Storage::StructuredStorage::{
        PROPVARIANT_0_0_0_abi, PROPVARIANT_0_0_abi, PROPVARIANT, PROPVARIANT_0, STGM_READ,
    },
    System::{
        Com::{CoCreateInstance, CLSCTX_ALL},
        OleAutomation::{VT_EMPTY, VT_LPWSTR},
        PropertiesSystem::{IPropertyStore, PROPERTYKEY},
        SystemServices::DEVPKEY_Device_FriendlyName,
    },
};
use crate::bindings::*;
use bimap::BiHashMap;
use smol::channel::{Receiver, Sender};
use smol::future::FutureExt;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use widestring::{U16CStr, U16CString};
use windows::{implement, Abi, Interface};

pub struct WindowsAudioBackend(());

impl WindowsAudioBackend {
    pub fn new() -> Self {
        Self(())
    }
}

impl AudioBackend for WindowsAudioBackend {
    type Error = windows::Error;

    fn start(self, handle: AudioHandle) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>>>> {
        Box::pin(async {
            let mut runtime = Runtime::new(self, handle).await?;
            runtime.run().await?;
            Ok(())
        })
    }
}

struct Runtime {
    handle: AudioHandle,
    device_enumerator: IMMDeviceEnumerator,
    event_rx: Receiver<NotifyEvent>,
    event_tx: Sender<NotifyEvent>,
    device_ids: BiHashMap<StreamId, DeviceId>,
    devices: HashMap<StreamId, AudioDevice>,
}

impl Runtime {
    async fn new(backend: WindowsAudioBackend, handle: AudioHandle) -> windows::Result<Self> {
        let _ = backend;

        let device_enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };

        // This should be unbounded, because the notifier does not run in an
        // async context and should not have to deal with a full queue.
        // In the current callback implementation, the callback would just drop
        // the message if the queue is bounded and full, which is also bad.
        let (event_tx, event_rx) = smol::channel::unbounded();

        Ok(Self {
            handle,
            device_enumerator,
            event_rx,
            event_tx,
            device_ids: BiHashMap::new(),
            devices: HashMap::new(),
        })
    }

    async fn run(&mut self) -> windows::Result<()> {
        // Init:

        // Register notifier to handle added/removed devices.
        let notifier = IMMNotificationClient::from(AudioNotifier {
            event_tx: self.event_tx.clone(),
        });
        unsafe {
            self.device_enumerator
                .RegisterEndpointNotificationCallback(&notifier)?
        };
        // Reference counting is not handled automatically by register func.
        // Release() gets called when dropped, prevent that from happening:
        std::mem::forget(notifier);

        let device_list = unsafe {
            self.device_enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?
        };
        let num_devices = unsafe { device_list.GetCount()? };
        for index in 0..num_devices {
            let ll_device = unsafe { device_list.Item(index)? };
            self.add_device(ll_device).await?;
        }
        self.add_device(unsafe {
            self.device_enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)?
        })
        .await?;

        loop {
            let control_future = async { self.handle.recv().await.map(Incoming::Control) };
            let notify_future = async {
                Some(Incoming::Notify(
                    self.event_rx.recv().await.expect("notifier hung up"),
                ))
            };
            let incoming = control_future.or(notify_future).await;
            log::debug!("incoming {:?}", incoming);
            match incoming {
                Some(Incoming::Control(control_message)) => match control_message {
                    AudioControl::StreamControl {
                        stream_id,
                        stream_control,
                    } => {
                        if let Some(device) = self.devices.get(&stream_id) {
                            match stream_control {
                                StreamControl::SetVolume(volume) => {
                                    device.set_volume(volume)?;
                                }
                                StreamControl::SetMuted(muted) => {
                                    device.set_muted(muted)?;
                                }
                                StreamControl::ToggleMuted => {
                                    device.toggle_muted()?;
                                }
                                StreamControl::StepVolume(steps) => {
                                    device.step_volume(steps)?;
                                }
                            }
                        } else {
                            log::warn!("received control for unknown stream {:?}", stream_id);
                        }
                    }
                },
                Some(Incoming::Notify(notify_message)) => match notify_message {
                    NotifyEvent::DeviceAdded(device_id) => {
                        let ll_device =
                            unsafe { self.device_enumerator.GetDevice(device_id.as_pwstr())? };
                        self.add_device(ll_device).await?;
                    }
                    NotifyEvent::DeviceRemoved(device_id) => {
                        if let Some((stream_id, _)) = self.device_ids.remove_by_right(&device_id) {
                            self.handle
                                .send(AudioEvent::StreamClosed { stream_id })
                                .await;
                            self.devices.remove(&stream_id);
                        }
                    }
                    NotifyEvent::DeviceStateChanged(device_id, device_state) => {
                        if let Some(&stream_id) = self.device_ids.get_by_right(&device_id) {
                            match device_state {
                                DeviceState::Active => {
                                    let device = &self.devices[&stream_id];
                                    self.handle
                                        .send(AudioEvent::StreamOpened {
                                            stream_id,
                                            name: device.name()?,
                                            volume: device.volume()?,
                                            muted: device.is_muted()?,
                                        })
                                        .await;
                                }
                                _ => {
                                    self.handle
                                        .send(AudioEvent::StreamClosed { stream_id })
                                        .await;
                                }
                            }
                        }
                    }
                    NotifyEvent::VolumeChanged(stream_id, volume) => {
                        self.handle
                            .send(AudioEvent::StreamEvent {
                                stream_id,
                                stream_event: StreamEvent::VolumeChanged(volume),
                            })
                            .await;
                    }
                    NotifyEvent::MutedChanged(stream_id, muted) => {
                        self.handle
                            .send(AudioEvent::StreamEvent {
                                stream_id,
                                stream_event: StreamEvent::MutedChanged(muted),
                            })
                            .await;
                    }
                },
                None => break,
            }
        }

        Ok(())
    }

    async fn add_device(&mut self, ll_device: IMMDevice) -> windows::Result<()> {
        let device_info = DeviceInfo::new(&ll_device)?;
        if self.device_ids.contains_right(&device_info.id) {
            return Ok(());
        }
        log::debug!("Registering device {:?}", device_info);
        let device = match AudioDevice::new(ll_device.clone(), self.event_tx.clone()) {
            Ok(x) => x,
            Err(e) => {
                log::warn!("could not open audio device: {}", e);
                return Ok(());
            }
        };
        let stream_id = device.stream_id();
        let name = device.name()?;
        let volume = device.volume()?;
        let muted = device.is_muted()?;
        let device_id = device.id()?;
        self.device_ids
            .insert_no_overwrite(stream_id, device_id)
            .expect("device id conflict");
        self.devices.insert(stream_id, device);
        self.handle
            .send(AudioEvent::StreamOpened {
                stream_id,
                name,
                volume,
                muted,
            })
            .await;
        Ok(())
    }
}

/// Enumeration of all incoming message types, for parallel awaiting.
#[derive(Debug)]
enum Incoming {
    Control(AudioControl),
    Notify(NotifyEvent),
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct DeviceId(U16CString);

impl DeviceId {
    unsafe fn new(pwstr: PWSTR) -> Option<Self> {
        if pwstr.is_null() {
            None
        } else {
            Some(DeviceId(U16CStr::from_ptr_str(pwstr.0).to_ucstring()))
        }
    }

    fn as_pwstr(&self) -> PWSTR {
        PWSTR(self.0.as_ptr() as *mut _)
    }
}

impl std::fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("DeviceId")
            .field(&self.0.to_string_lossy())
            .finish()
    }
}

#[derive(Debug)]
struct DeviceInfo {
    id: DeviceId,
    name: String,
}

impl DeviceInfo {
    fn new(device: &IMMDevice) -> windows::Result<Self> {
        Ok(Self {
            id: unsafe { DeviceId::new(device.GetId()?).expect("null pointer") },
            name: unsafe {
                U16CStr::from_ptr_str(
                    Property::from(
                        device
                            .OpenPropertyStore(STGM_READ as _)?
                            .GetValue(&DEVPKEY_Device_FriendlyName)?,
                    )
                    .as_pwstr()
                    .expect("invalid type")
                    .0,
                )
                .to_string_lossy()
            },
        })
    }
}

#[implement(Windows::Win32::Media::Audio::CoreAudio::IMMNotificationClient)]
struct AudioNotifier {
    event_tx: Sender<NotifyEvent>,
}

// Impl IMMNotificationClient
#[allow(non_snake_case)]
impl AudioNotifier {
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        device_id: PWSTR,
    ) -> windows::Result<()> {
        let _ = (flow, role, device_id);
        Ok(())
    }

    fn OnDeviceAdded(&self, device_id: PWSTR) -> windows::Result<()> {
        let device_id = unsafe { DeviceId::new(device_id).expect("null") };
        self.event_tx
            .try_send(NotifyEvent::DeviceAdded(device_id))
            .ok();
        Ok(())
    }

    fn OnDeviceRemoved(&self, device_id: PWSTR) -> windows::Result<()> {
        let device_id = unsafe { DeviceId::new(device_id).expect("null") };
        self.event_tx
            .try_send(NotifyEvent::DeviceRemoved(device_id))
            .ok();
        Ok(())
    }

    fn OnDeviceStateChanged(&self, device_id: PWSTR, new_state: u32) -> windows::Result<()> {
        let device_id = unsafe { DeviceId::new(device_id).expect("null") };
        self.event_tx
            .try_send(NotifyEvent::DeviceStateChanged(
                device_id,
                DeviceState::parse(new_state).expect("unknown state"),
            ))
            .ok();
        Ok(())
    }

    fn OnPropertyValueChanged(&self, device_id: PWSTR, key: PROPERTYKEY) -> windows::Result<()> {
        let _ = (device_id, key);
        Ok(())
    }
}

#[derive(Debug)]
enum NotifyEvent {
    DeviceAdded(DeviceId),
    DeviceRemoved(DeviceId),
    DeviceStateChanged(DeviceId, DeviceState),
    VolumeChanged(StreamId, f32),
    MutedChanged(StreamId, bool),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DeviceState {
    /// The device is present and enabled.
    Active,
    /// The device is present but is disabled by the user.
    Disabled,
    /// The device is not present (disconnected from the system).
    NotPresent,
    /// The device is present and enabled, but it has jack-presence detection
    /// and nothing is plugged into its jack.
    Unplugged,
}

impl DeviceState {
    fn parse(x: u32) -> Option<Self> {
        if x == DEVICE_STATE_ACTIVE {
            Some(Self::Active)
        } else if x == DEVICE_STATE_DISABLED {
            Some(Self::Disabled)
        } else if x == DEVICE_STATE_NOTPRESENT {
            Some(Self::NotPresent)
        } else if x == DEVICE_STATE_UNPLUGGED {
            Some(Self::Unplugged)
        } else {
            None
        }
    }
}

struct AudioDevice {
    ll_device: IMMDevice,
    properties: IPropertyStore,
    volume: IAudioEndpointVolume,
    stream_id: StreamId,
}

impl AudioDevice {
    fn new(ll_device: IMMDevice, event_tx: Sender<NotifyEvent>) -> windows::Result<Self> {
        let properties = unsafe { ll_device.OpenPropertyStore(STGM_READ as _)? };
        let mut volume = None;
        unsafe {
            ll_device.Activate(
                &IAudioEndpointVolume::IID,
                0,
                std::ptr::null_mut(),
                volume.set_abi(),
            )?;
        }
        let volume: IAudioEndpointVolume = volume.expect("volume control creation failed");
        let stream_id = StreamId::new();

        let notifier = IAudioEndpointVolumeCallback::from(DeviceNotifier {
            stream_id,
            event_tx,
        });
        unsafe { volume.RegisterControlChangeNotify(notifier)? };

        Ok(Self {
            ll_device,
            properties,
            volume,
            stream_id,
        })
    }

    fn name(&self) -> windows::Result<String> {
        let variant = unsafe { self.properties.GetValue(&DEVPKEY_Device_FriendlyName)? };
        match Property::from(variant) {
            Property::Pwstr(pwstr) => {
                Ok(unsafe { U16CStr::from_ptr_str(pwstr.0).to_string_lossy() })
            }
            _ => unreachable!(),
        }
    }

    fn id(&self) -> windows::Result<DeviceId> {
        let id_pwstr = unsafe { self.ll_device.GetId()? };
        Ok(unsafe { DeviceId::new(id_pwstr).expect("null") })
    }

    fn volume(&self) -> windows::Result<f32> {
        unsafe { self.volume.GetMasterVolumeLevelScalar() }
    }

    fn set_volume(&self, volume: f32) -> windows::Result<()> {
        unsafe {
            self.volume
                .SetMasterVolumeLevelScalar(volume, std::ptr::null())
        }
    }

    fn is_muted(&self) -> windows::Result<bool> {
        unsafe { self.volume.GetMute().map(Into::into) }
    }

    fn set_muted(&self, muted: bool) -> windows::Result<()> {
        unsafe { self.volume.SetMute(muted, std::ptr::null()) }
    }

    fn toggle_muted(&self) -> windows::Result<()> {
        self.set_muted(!self.is_muted()?)
    }

    fn step_volume(&self, steps: i32) -> windows::Result<()> {
        if steps > 0 {
            for _ in 0..steps {
                unsafe { self.volume.VolumeStepUp(std::ptr::null())? };
            }
        } else if steps < 0 {
            for _ in steps..0 {
                unsafe { self.volume.VolumeStepDown(std::ptr::null())? };
            }
        }
        Ok(())
    }

    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
}

enum Property {
    Empty,
    Pwstr(PWSTR),
}

impl Property {
    fn as_pwstr(&self) -> Option<PWSTR> {
        match self {
            &Self::Pwstr(x) => Some(x),
            _ => None,
        }
    }
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

#[implement(Windows::Win32::Media::Audio::CoreAudio::IAudioEndpointVolumeCallback)]
struct DeviceNotifier {
    stream_id: StreamId,
    event_tx: Sender<NotifyEvent>,
}

// impl IAudioEndpointVolumeCallback
#[allow(non_snake_case)]
impl DeviceNotifier {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::Result<()> {
        let data = unsafe { &*data };
        self.event_tx
            .try_send(NotifyEvent::VolumeChanged(
                self.stream_id,
                data.fMasterVolume,
            ))
            .ok();
        self.event_tx
            .try_send(NotifyEvent::MutedChanged(
                self.stream_id,
                data.bMuted.into(),
            ))
            .ok();
        Ok(())
    }
}

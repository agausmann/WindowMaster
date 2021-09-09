use crate::audio::{
    AudioBackend, AudioControl, AudioEvent, AudioHandle, StreamControl, StreamEvent, StreamId,
    StreamInfo, StreamInfoBuilder, StreamState,
};
use crate::bindings::Windows::Win32::{
    Foundation::{BOOL, PWSTR},
    Media::Audio::CoreAudio::{
        eConsole, eRender, AudioSessionDisconnectReason, AudioSessionState, EDataFlow, ERole,
        IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioSessionControl,
        IAudioSessionControl2, IAudioSessionEvents, IAudioSessionManager2,
        IAudioSessionNotification, IMMDevice, IMMDeviceEnumerator, IMMNotificationClient,
        ISimpleAudioVolume, MMDeviceEnumerator, AUDIO_VOLUME_NOTIFICATION_DATA,
        DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED, DEVICE_STATE_NOTPRESENT,
        DEVICE_STATE_UNPLUGGED,
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
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use widestring::{U16CStr, U16CString};
use windows::{implement, Abi, Guid, Interface};

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
    session_ids: BiHashMap<StreamId, SessionId>,
    devices: HashMap<StreamId, AudioDevice>,
    sessions: HashMap<StreamId, AudioSession>,
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
            session_ids: BiHashMap::new(),
            devices: HashMap::new(),
            sessions: HashMap::new(),
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
                        } else if let Some(session) = self.sessions.get(&stream_id) {
                            match stream_control {
                                StreamControl::SetVolume(volume) => {
                                    session.set_volume(volume)?;
                                }
                                StreamControl::SetMuted(muted) => {
                                    session.set_muted(muted)?;
                                }
                                StreamControl::ToggleMuted => {
                                    session.toggle_muted()?;
                                }
                                StreamControl::StepVolume(steps) => {
                                    session.step_volume(steps)?;
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
                                            stream_info: device.stream_info()?,
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
                    NotifyEvent::StreamStateChanged(stream_id, state) => {
                        self.handle
                            .send(AudioEvent::StreamEvent {
                                stream_id,
                                stream_event: StreamEvent::StateChanged(state),
                            })
                            .await;
                    }
                    NotifyEvent::SessionCreated(parent_stream_id, session_id) => {
                        if !self.session_ids.contains_right(&session_id) {
                            if let Some(parent_device) = self.devices.get(&parent_stream_id) {
                                if let Some(session_control) =
                                    parent_device.find_session(&session_id)?
                                {
                                    let session = AudioSession::new(
                                        parent_stream_id,
                                        session_control,
                                        self.event_tx.clone(),
                                    )?;
                                    let stream_id = session.stream_id();
                                    let stream_info = session.stream_info()?;
                                    self.sessions.insert(stream_id, session);
                                    self.session_ids.insert(stream_id, session_id);
                                    self.handle
                                        .send(AudioEvent::StreamOpened {
                                            stream_id,
                                            stream_info,
                                        })
                                        .await;
                                }
                            }
                        }
                    }
                    NotifyEvent::SessionDisconnected(stream_id) => {
                        if self.sessions.remove(&stream_id).is_some() {
                            self.session_ids.remove_by_left(&stream_id);
                            self.handle
                                .send(AudioEvent::StreamClosed { stream_id })
                                .await;
                        };
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
        let device = match AudioDevice::new(ll_device.clone(), self.event_tx.clone()).await {
            Ok(x) => x,
            Err(e) => {
                log::warn!("could not open audio device: {}", e);
                return Ok(());
            }
        };
        let stream_id = device.stream_id();
        self.handle
            .send(AudioEvent::StreamOpened {
                stream_id,
                stream_info: device.stream_info()?,
            })
            .await;
        let device_id = device.id()?;
        self.device_ids
            .insert_no_overwrite(stream_id, device_id)
            .expect("device id conflict");
        self.devices.insert(stream_id, device);
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

#[derive(Clone, PartialEq, Eq, Hash)]
struct SessionId(U16CString);

impl SessionId {
    unsafe fn new(pwstr: PWSTR) -> Option<Self> {
        if pwstr.is_null() {
            None
        } else {
            Some(SessionId(U16CStr::from_ptr_str(pwstr.0).to_ucstring()))
        }
    }

    fn as_pwstr(&self) -> PWSTR {
        PWSTR(self.0.as_ptr() as *mut _)
    }

    fn as_ucstr(&self) -> &U16CStr {
        self.0.as_ucstr()
    }
}

impl std::fmt::Debug for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("SessionId")
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

impl AudioNotifier {
    fn send(&self, event: NotifyEvent) {
        smol::block_on(self.event_tx.send(event)).ok();
    }
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
        self.send(NotifyEvent::DeviceAdded(device_id));
        Ok(())
    }

    fn OnDeviceRemoved(&self, device_id: PWSTR) -> windows::Result<()> {
        let device_id = unsafe { DeviceId::new(device_id).expect("null") };
        self.send(NotifyEvent::DeviceRemoved(device_id));
        Ok(())
    }

    fn OnDeviceStateChanged(&self, device_id: PWSTR, new_state: u32) -> windows::Result<()> {
        let device_id = unsafe { DeviceId::new(device_id).expect("null") };
        self.send(NotifyEvent::DeviceStateChanged(
            device_id,
            DeviceState::parse(new_state).expect("unknown state"),
        ));
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
    StreamStateChanged(StreamId, StreamState),
    SessionCreated(StreamId, SessionId),
    SessionDisconnected(StreamId),
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

struct AudioSession {
    parent_stream_id: StreamId,
    stream_id: StreamId,
    session_control: IAudioSessionControl2,
    volume_control: ISimpleAudioVolume,
}

impl AudioSession {
    fn new(
        parent_stream_id: StreamId,
        session_control: IAudioSessionControl2,
        event_tx: Sender<NotifyEvent>,
    ) -> windows::Result<Self> {
        let volume_control: ISimpleAudioVolume = session_control.cast()?;

        let stream_id = StreamId::new();

        let session_notifier = IAudioSessionEvents::from(SessionNotifier {
            stream_id,
            event_tx,
        });
        unsafe { session_control.RegisterAudioSessionNotification(session_notifier)? };

        Ok(Self {
            parent_stream_id,
            stream_id,
            session_control,
            volume_control,
        })
    }

    fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    fn name(&self) -> windows::Result<String> {
        // if unsafe { self.session_control.IsSystemSoundsSession() } {
        //     return Ok("System Sounds".into());
        // }
        let mut name = unsafe {
            U16CStr::from_ptr_str(self.session_control.GetDisplayName()?.0).to_string_lossy()
        };
        if name.is_empty() {
            name = get_process_name(&self.session_control).unwrap_or(String::new())
        }
        //XXX temporary fix until I can read the return code of IsSystemSoundsSession
        // https://github.com/microsoft/windows-rs/issues/1065
        if name.contains("AudioSrv.Dll") {
            return Ok("System Sounds".into());
        }
        Ok(name)
    }

    fn volume(&self) -> windows::Result<f32> {
        unsafe { self.volume_control.GetMasterVolume() }
    }

    fn muted(&self) -> windows::Result<bool> {
        unsafe { self.volume_control.GetMute().map(Into::into) }
    }

    fn stream_state(&self) -> windows::Result<StreamState> {
        Ok(StreamState {
            volume: self.volume()?,
            muted: self.muted()?,
        })
    }

    fn stream_info(&self) -> windows::Result<StreamInfo> {
        Ok(StreamInfoBuilder::new(self.name()?)
            .with_initial_state(self.stream_state()?)
            .with_parent(self.parent_stream_id)
            .build())
    }

    fn set_volume(&self, volume: f32) -> windows::Result<()> {
        unsafe {
            self.volume_control
                .SetMasterVolume(volume, std::ptr::null())
        }
    }

    fn set_muted(&self, muted: bool) -> windows::Result<()> {
        unsafe { self.volume_control.SetMute(muted, std::ptr::null()) }
    }

    fn toggle_muted(&self) -> windows::Result<()> {
        unsafe {
            self.volume_control
                .SetMute(!self.muted()?, std::ptr::null())
        }
    }

    pub(crate) fn step_volume(&self, steps: i32) -> windows::Result<()> {
        unsafe {
            self.volume_control.SetMasterVolume(
                (self.volume()? + steps as f32 * 0.02).clamp(0.0, 1.0),
                std::ptr::null(),
            )
        }
    }
}

struct AudioDevice {
    stream_id: StreamId,
    ll_device: IMMDevice,
    properties: IPropertyStore,
    volume: IAudioEndpointVolume,
    session_manager: IAudioSessionManager2,
}

impl AudioDevice {
    async fn new(ll_device: IMMDevice, event_tx: Sender<NotifyEvent>) -> windows::Result<Self> {
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

        let mut session_manager = None;
        unsafe {
            ll_device.Activate(
                &IAudioSessionManager2::IID,
                0,
                std::ptr::null_mut(),
                session_manager.set_abi(),
            )?;
        }
        let session_manager: IAudioSessionManager2 =
            session_manager.expect("session manager creation failed");

        let stream_id = StreamId::new();

        let session_enumerator = unsafe { session_manager.GetSessionEnumerator()? };
        let num_sessions = unsafe { session_enumerator.GetCount()? };
        for i in 0..num_sessions {
            let session_control = unsafe { session_enumerator.GetSession(i)? };
            let session_control: IAudioSessionControl2 = session_control.cast()?;
            let session_id =
                unsafe { SessionId::new(session_control.GetSessionIdentifier()?).expect("null") };

            event_tx
                .send(NotifyEvent::SessionCreated(stream_id, session_id))
                .await
                .ok();
        }

        let device_notifier = IAudioEndpointVolumeCallback::from(DeviceNotifier {
            stream_id,
            event_tx: event_tx.clone(),
        });
        unsafe { volume.RegisterControlChangeNotify(device_notifier)? };

        let session_notifier = IAudioSessionNotification::from(NewSessionNotifier {
            stream_id,
            event_tx,
        });
        unsafe { session_manager.RegisterSessionNotification(session_notifier)? };

        Ok(Self {
            ll_device,
            properties,
            volume,
            stream_id,
            session_manager,
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

    fn stream_state(&self) -> windows::Result<StreamState> {
        Ok(StreamState {
            volume: self.volume()?,
            muted: self.is_muted()?,
        })
    }

    fn stream_info(&self) -> windows::Result<StreamInfo> {
        Ok(StreamInfoBuilder::new(self.name()?)
            .with_initial_state(self.stream_state()?)
            .build())
    }

    fn find_session(
        &self,
        session_id: &SessionId,
    ) -> windows::Result<Option<IAudioSessionControl2>> {
        let enumerator = unsafe { self.session_manager.GetSessionEnumerator()? };
        let num_sessions = unsafe { enumerator.GetCount()? };
        for i in 0..num_sessions {
            let session_control = unsafe { enumerator.GetSession(i)? };
            let session_control: IAudioSessionControl2 = session_control.cast()?;
            let id = unsafe { U16CStr::from_ptr_str(session_control.GetSessionIdentifier()?.0) };
            if id == session_id.as_ucstr() {
                return Ok(Some(session_control));
            }
        }
        Ok(None)
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

impl DeviceNotifier {
    fn send(&self, event: NotifyEvent) {
        smol::block_on(self.event_tx.send(event)).ok();
    }
}

// impl IAudioEndpointVolumeCallback
#[allow(non_snake_case)]
impl DeviceNotifier {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::Result<()> {
        let data = unsafe { &*data };
        self.send(NotifyEvent::StreamStateChanged(
            self.stream_id,
            StreamState {
                volume: data.fMasterVolume,
                muted: data.bMuted.into(),
            },
        ));
        Ok(())
    }
}

#[implement(Windows::Win32::Media::Audio::CoreAudio::IAudioSessionNotification)]
struct NewSessionNotifier {
    stream_id: StreamId,
    event_tx: Sender<NotifyEvent>,
}

impl NewSessionNotifier {
    fn send(&self, event: NotifyEvent) {
        smol::block_on(self.event_tx.send(event)).ok();
    }
}

// impl IAudioSessionNotification
#[allow(non_snake_case)]
impl NewSessionNotifier {
    fn OnSessionCreated(&self, new_session: &Option<IAudioSessionControl>) -> windows::Result<()> {
        if let Some(session) = new_session {
            let session: IAudioSessionControl2 = session.cast()?;
            let session_id =
                unsafe { SessionId::new(session.GetSessionIdentifier()?).expect("null") };
            self.send(NotifyEvent::SessionCreated(self.stream_id, session_id))
        }
        Ok(())
    }
}

#[implement(Windows::Win32::Media::Audio::CoreAudio::IAudioSessionEvents)]
struct SessionNotifier {
    stream_id: StreamId,
    event_tx: Sender<NotifyEvent>,
}

impl Drop for SessionNotifier {
    fn drop(&mut self) {
        log::warn!("dropped {:?}", self.stream_id);
    }
}

impl SessionNotifier {
    fn send(&self, event: NotifyEvent) {
        smol::block_on(self.event_tx.send(event)).ok();
    }
}

// impl IAudioSessionEvents
#[allow(non_snake_case)]
impl SessionNotifier {
    fn OnDisplayNameChanged(
        &self,
        new_display_name: PWSTR,
        event_context: *const Guid,
    ) -> windows::Result<()> {
        let _ = event_context;
        Ok(())
    }

    fn OnIconPathChanged(
        &self,
        new_icon_path: PWSTR,
        event_context: *const Guid,
    ) -> windows::Result<()> {
        let _ = event_context;
        Ok(())
    }

    fn OnSimpleVolumeChanged(
        &self,
        new_volume: f32,
        new_mute: BOOL,
        event_context: *const Guid,
    ) -> windows::Result<()> {
        let _ = event_context;
        self.send(NotifyEvent::StreamStateChanged(
            self.stream_id,
            StreamState {
                volume: new_volume,
                muted: new_mute.into(),
            },
        ));
        Ok(())
    }

    fn OnChannelVolumeChanged(
        &self,
        channel_count: u32,
        new_channel_volume_array: *mut f32,
        changed_channel: u32,
        event_context: *const Guid,
    ) -> windows::Result<()> {
        let _ = (
            channel_count,
            new_channel_volume_array,
            changed_channel,
            event_context,
        );
        Ok(())
    }

    fn OnGroupingParamChanged(
        &self,
        new_grouping_param: *const Guid,
        event_context: *const Guid,
    ) -> windows::Result<()> {
        let _ = (new_grouping_param, event_context);
        Ok(())
    }

    fn OnStateChanged(&self, new_state: AudioSessionState) -> windows::Result<()> {
        Ok(())
    }

    fn OnSessionDisconnected(
        &self,
        disconnect_reason: AudioSessionDisconnectReason,
    ) -> windows::Result<()> {
        self.send(NotifyEvent::SessionDisconnected(self.stream_id));
        Ok(())
    }
}

fn get_process_name(session_control: &IAudioSessionControl2) -> Option<String> {
    let string = unsafe {
        U16CStr::from_ptr_str(session_control.GetSessionIdentifier().ok()?.0).to_string_lossy()
    };

    let file_name = string.rsplit_once('%')?.0.rsplit_once('\\')?.1;

    Some(
        file_name
            .rsplit_once('.')
            .map(|(stem, _extension)| stem)
            .unwrap_or(file_name)
            .to_string(),
    )
}

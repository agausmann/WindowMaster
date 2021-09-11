use crate::audio::{
    AudioBackend, AudioControl, AudioEvent, AudioHandle, StreamControl, StreamEvent, StreamId,
    StreamInfo, StreamInfoBuilder, StreamState,
};
use bimap::BiHashMap;
use smol::{
    channel::{Receiver, Sender},
    future::FutureExt,
};
use std::{collections::HashMap, fmt::Debug, future::Future, pin::Pin};
use win32_coreaudio::{
    string::{WinStr, WinString},
    AudioEndpointVolume, AudioEndpointVolumeCallback, AudioSessionControl, AudioSessionControl2,
    AudioSessionDisconnectReason, AudioSessionEvents, AudioSessionManager2,
    AudioSessionNotification, DataFlowMask, Device, DeviceEnumerator, DeviceState, DeviceStateMask,
    NotificationClient, NotificationData, Property, PropertyStore, SimpleAudioVolume,
    StorageAccessMode, DEVICE_FRIENDLY_NAME,
};
use windows::Guid;

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
    device_enumerator: DeviceEnumerator,
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

        let device_enumerator = DeviceEnumerator::new()?;

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
        let notifier = AudioNotifier {
            event_tx: self.event_tx.clone(),
        };
        self.device_enumerator
            .register_endpoint_notification(notifier)?;

        let device_list = self
            .device_enumerator
            .enum_audio_endpoints(DataFlowMask::Render, DeviceStateMask::ACTIVE)?;
        for ll_device in &device_list {
            self.add_device(ll_device).await?;
        }

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
                        let ll_device = self.device_enumerator.get_device(device_id.as_winstr())?;
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

    async fn add_device(&mut self, ll_device: Device) -> windows::Result<()> {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DeviceId(WinString);

impl DeviceId {
    fn as_winstr(&self) -> &WinStr {
        self.0.as_winstr()
    }
}

impl From<&WinStr> for DeviceId {
    fn from(winstr: &WinStr) -> Self {
        Self(winstr.to_winstring())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SessionId(WinString);

impl From<&WinStr> for SessionId {
    fn from(winstr: &WinStr) -> Self {
        Self(winstr.to_winstring())
    }
}

#[derive(Debug)]
struct DeviceInfo {
    id: DeviceId,
    name: String,
}

impl DeviceInfo {
    fn new(device: &Device) -> windows::Result<Self> {
        Ok(Self {
            id: DeviceId(device.get_id()?),
            name: device
                .open_property_store(StorageAccessMode::Read)?
                .get_value(&DEVICE_FRIENDLY_NAME)
                .map(string_prop)?,
        })
    }
}

struct AudioNotifier {
    event_tx: Sender<NotifyEvent>,
}

impl AudioNotifier {
    fn send(&self, event: NotifyEvent) {
        smol::block_on(self.event_tx.send(event)).ok();
    }
}

impl NotificationClient for AudioNotifier {
    fn on_device_added(&mut self, device_id: &WinStr) -> windows::Result<()> {
        self.send(NotifyEvent::DeviceAdded(device_id.into()));
        Ok(())
    }

    fn on_device_removed(&mut self, device_id: &WinStr) -> windows::Result<()> {
        self.send(NotifyEvent::DeviceRemoved(device_id.into()));
        Ok(())
    }

    fn on_device_state_changed(
        &mut self,
        device_id: &WinStr,
        state: DeviceState,
    ) -> windows::Result<()> {
        self.send(NotifyEvent::DeviceStateChanged(device_id.into(), state));
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

struct AudioSession {
    parent_stream_id: StreamId,
    stream_id: StreamId,
    session_control: AudioSessionControl2,
    volume_control: SimpleAudioVolume,
}

impl AudioSession {
    fn new(
        parent_stream_id: StreamId,
        session_control: AudioSessionControl2,
        event_tx: Sender<NotifyEvent>,
    ) -> windows::Result<Self> {
        let volume_control: SimpleAudioVolume = session_control.get_simple_audio_volume()?;

        let stream_id = StreamId::new();

        let session_notifier = SessionNotifier {
            stream_id,
            event_tx,
        };
        session_control.register_audio_session_notification(session_notifier)?;

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
        // if self.session_control.is_system_sounds_session() {
        //     return Ok("System Sounds".into());
        // }
        let mut name = self.session_control.get_display_name()?.to_string_lossy();
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
        self.volume_control.get_master_volume()
    }

    fn muted(&self) -> windows::Result<bool> {
        self.volume_control.get_mute()
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
        self.volume_control.set_master_volume(volume, None)
    }

    fn set_muted(&self, muted: bool) -> windows::Result<()> {
        self.volume_control.set_mute(muted, None)
    }

    fn toggle_muted(&self) -> windows::Result<()> {
        self.set_muted(!self.muted()?)
    }

    fn step_volume(&self, steps: i32) -> windows::Result<()> {
        self.set_volume((self.volume()? + steps as f32 * 0.02).clamp(0.0, 1.0))
    }
}

struct AudioDevice {
    stream_id: StreamId,
    ll_device: Device,
    properties: PropertyStore,
    volume: AudioEndpointVolume,
    session_manager: AudioSessionManager2,
}

impl AudioDevice {
    async fn new(ll_device: Device, event_tx: Sender<NotifyEvent>) -> windows::Result<Self> {
        let properties = ll_device.open_property_store(StorageAccessMode::Read)?;
        let volume = ll_device.activate_audio_endpoint_volume()?;
        let session_manager = ll_device.activate_audio_session_manager2()?;

        let stream_id = StreamId::new();

        let session_enumerator = session_manager.get_session_enumerator()?;
        for session_control in &session_enumerator {
            let session_control = session_control.upgrade()?;
            let session_id = SessionId(session_control.get_session_identifier()?);

            event_tx
                .send(NotifyEvent::SessionCreated(stream_id, session_id))
                .await
                .ok();
        }

        let device_notifier = DeviceNotifier {
            stream_id,
            event_tx: event_tx.clone(),
        };
        volume.register_control_change_notify(device_notifier)?;

        let session_notifier = NewSessionNotifier {
            stream_id,
            event_tx,
        };
        session_manager.register_session_notification(session_notifier)?;

        Ok(Self {
            ll_device,
            properties,
            volume,
            stream_id,
            session_manager,
        })
    }

    fn name(&self) -> windows::Result<String> {
        self.properties
            .get_value(&DEVICE_FRIENDLY_NAME)
            .map(string_prop)
    }

    fn id(&self) -> windows::Result<DeviceId> {
        self.ll_device.get_id().map(DeviceId)
    }

    fn volume(&self) -> windows::Result<f32> {
        self.volume.get_master_volume_level_scalar()
    }

    fn set_volume(&self, volume: f32) -> windows::Result<()> {
        self.volume.set_master_volume_level_scalar(volume, None)
    }

    fn is_muted(&self) -> windows::Result<bool> {
        self.volume.get_mute()
    }

    fn set_muted(&self, muted: bool) -> windows::Result<()> {
        self.volume.set_mute(muted, None)
    }

    fn toggle_muted(&self) -> windows::Result<()> {
        self.set_muted(!self.is_muted()?)
    }

    fn step_volume(&self, steps: i32) -> windows::Result<()> {
        if steps > 0 {
            for _ in 0..steps {
                self.volume.volume_step_up(None)?;
            }
        } else if steps < 0 {
            for _ in steps..0 {
                self.volume.volume_step_down(None)?;
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
    ) -> windows::Result<Option<AudioSessionControl2>> {
        let session_enumerator = self.session_manager.get_session_enumerator()?;
        for session_control in &session_enumerator {
            let session_control = session_control.upgrade()?;
            let id = SessionId(session_control.get_session_identifier()?);
            if id == *session_id {
                return Ok(Some(session_control));
            }
        }
        Ok(None)
    }
}

fn string_prop(property: Property) -> String {
    match property {
        Property::Str(winstring) => winstring.to_string_lossy(),
        _ => panic!("invalid type {:?}", property),
    }
}

struct DeviceNotifier {
    stream_id: StreamId,
    event_tx: Sender<NotifyEvent>,
}

impl DeviceNotifier {
    fn send(&self, event: NotifyEvent) {
        smol::block_on(self.event_tx.send(event)).ok();
    }
}

impl AudioEndpointVolumeCallback for DeviceNotifier {
    fn on_notify(&mut self, data: &NotificationData) -> windows::Result<()> {
        self.send(NotifyEvent::StreamStateChanged(
            self.stream_id,
            StreamState {
                volume: data.master_volume,
                muted: data.muted,
            },
        ));
        Ok(())
    }
}

struct NewSessionNotifier {
    stream_id: StreamId,
    event_tx: Sender<NotifyEvent>,
}

impl NewSessionNotifier {
    fn send(&self, event: NotifyEvent) {
        smol::block_on(self.event_tx.send(event)).ok();
    }
}

impl AudioSessionNotification for NewSessionNotifier {
    fn on_session_created(&mut self, new_session: AudioSessionControl) -> windows::Result<()> {
        let new_session = new_session.upgrade()?;
        let session_id = SessionId(new_session.get_session_identifier()?);
        self.send(NotifyEvent::SessionCreated(self.stream_id, session_id));
        Ok(())
    }
}

struct SessionNotifier {
    stream_id: StreamId,
    event_tx: Sender<NotifyEvent>,
}

impl SessionNotifier {
    fn send(&self, event: NotifyEvent) {
        smol::block_on(self.event_tx.send(event)).ok();
    }
}

impl AudioSessionEvents for SessionNotifier {
    fn on_simple_volume_changed(
        &mut self,
        new_volume: f32,
        new_mute: bool,
        event_context: Option<&Guid>,
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

    fn on_session_disconnected(
        &mut self,
        disconnect_reason: AudioSessionDisconnectReason,
    ) -> windows::Result<()> {
        let _ = disconnect_reason;
        self.send(NotifyEvent::SessionDisconnected(self.stream_id));
        Ok(())
    }
}

fn get_process_name(session_control: &AudioSessionControl2) -> Option<String> {
    let string = session_control
        .get_session_identifier()
        .ok()?
        .to_string_lossy();
    let file_name = string.rsplit_once('%')?.0.rsplit_once('\\')?.1;
    Some(
        file_name
            .rsplit_once('.')
            .map(|(stem, _extension)| stem)
            .unwrap_or(file_name)
            .to_string(),
    )
}

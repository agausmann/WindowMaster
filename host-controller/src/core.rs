use crate::{
    audio::{
        AudioBackend, AudioControl, AudioEvent, AudioHandle, StreamControl, StreamId, StreamState,
    },
    bigraph::BiGraph,
    control::{
        ChannelInput, ChannelOutput, ControlBackend, ControlHandle, ControlInput, ControlOutput,
        DeviceId,
    },
};
use smol::{
    channel::{Receiver, Sender},
    future::FutureExt,
};
use std::collections::HashMap;

pub struct Core<A, C> {
    audio_backend: A,
    control_backend: C,
}

impl<A, C> Core<A, C>
where
    A: AudioBackend,
    C: ControlBackend,
{
    pub fn new(audio_backend: A, control_backend: C) -> Self {
        Self {
            audio_backend,
            control_backend,
        }
    }

    pub fn run(self) -> Result<(), Box<dyn std::error::Error + 'static>> {
        let Self {
            audio_backend,
            control_backend,
        } = self;

        let (audio_event_tx, audio_event_rx) = smol::channel::unbounded();
        let (audio_control_tx, audio_control_rx) = smol::channel::unbounded();
        let audio_handle = AudioHandle::new(audio_event_tx, audio_control_rx);
        let audio_task = async {
            let result = audio_backend.start(audio_handle).await;
            log::warn!("audio task exited: {:?}", result);
            result
        };

        let (control_input_tx, control_input_rx) = smol::channel::unbounded();
        let (control_output_tx, control_output_rx) = smol::channel::unbounded();
        let control_handle = ControlHandle::new(control_input_tx, control_output_rx);
        let control_task = async {
            let result = control_backend.start(control_handle).await;
            log::warn!("control task exited: {:?}", result);
            result
        };

        let mut runtime = Runtime {
            audio_event_rx,
            audio_control_tx,
            control_input_rx,
            control_output_tx,
            streams: HashMap::new(),
            bindings: BiGraph::new(),
            menus: HashMap::new(),
            window_focus: None,
            default_device: None,
        };
        let runtime_task = runtime.run();

        use smol::future::zip;
        let ((audio_result, control_result), _) =
            smol::block_on(zip(zip(audio_task, control_task), runtime_task));
        audio_result?;
        control_result?;
        Ok(())
    }
}

struct Runtime {
    audio_event_rx: Receiver<AudioEvent>,
    audio_control_tx: Sender<AudioControl>,
    control_input_rx: Receiver<ControlInput>,
    control_output_tx: Sender<ControlOutput>,
    streams: HashMap<StreamId, Stream>,
    bindings: BiGraph<ChannelId, Binding>,
    menus: HashMap<ChannelId, Menu>,
    window_focus: Option<StreamId>,
    default_device: Option<StreamId>,
}

impl Runtime {
    async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let audio_event_task = async {
                self.audio_event_rx
                    .recv()
                    .await
                    .ok()
                    .map(Incoming::AudioEvent)
            };
            let control_input_task = async {
                self.control_input_rx
                    .recv()
                    .await
                    .ok()
                    .map(Incoming::ControlInput)
            };
            let incoming = audio_event_task.or(control_input_task).await;
            log::debug!("incoming {:?}", incoming);
            match incoming {
                Some(Incoming::AudioEvent(audio_event)) => match audio_event {
                    AudioEvent::StreamOpened {
                        stream_id,
                        stream_info,
                    } => {
                        self.streams.insert(
                            stream_id,
                            Stream {
                                name: stream_info.name().to_string(),
                                state: stream_info.initial_state(),
                            },
                        );
                    }
                    AudioEvent::StreamClosed { stream_id } => {
                        self.streams.remove(&stream_id);
                    }
                    AudioEvent::StreamEvent {
                        stream_id,
                        stream_event,
                    } => match stream_event {
                        crate::audio::StreamEvent::StateChanged(state) => {
                            if let Some(stream) = self.streams.get_mut(&stream_id) {
                                stream.state = state;
                                self.update_bound_channels(Binding::Direct(stream_id))
                                    .await?;
                                if self.window_focus == Some(stream_id) {
                                    self.update_bound_channels(Binding::ActiveWindow).await?;
                                }
                            }
                        }
                    },
                    AudioEvent::WindowFocusChanged { stream_id } => {
                        self.window_focus = stream_id;
                        self.update_bound_channels(Binding::ActiveWindow).await?;
                    }
                    AudioEvent::DefaultDeviceChanged { stream_id } => {
                        self.default_device = stream_id;
                        self.update_bound_channels(Binding::DefaultDevice).await?;
                    }
                },
                Some(Incoming::ControlInput(control_input)) => match control_input {
                    ControlInput::DeviceAdded(device_id, device_info) => {
                        let _ = (device_id, device_info);
                    }
                    ControlInput::DeviceRemoved(device_id) => {
                        let _ = device_id;
                    }
                    ControlInput::ChannelInput(device_id, channel_index, channel_input) => {
                        let channel_id = ChannelId(device_id, channel_index);
                        match channel_input {
                            ChannelInput::SetVolume(volume) => {
                                for binding in self.bindings.neighbors_of_left(channel_id) {
                                    if let Some(stream_id) = self.binding_stream_id(&binding) {
                                        self.audio_control_tx
                                            .send(AudioControl::StreamControl {
                                                stream_id,
                                                stream_control: StreamControl::SetVolume(volume),
                                            })
                                            .await?;
                                    }
                                }
                            }
                            ChannelInput::StepVolume(steps) => {
                                for binding in self.bindings.neighbors_of_left(channel_id) {
                                    if let Some(stream_id) = self.binding_stream_id(&binding) {
                                        self.audio_control_tx
                                            .send(AudioControl::StreamControl {
                                                stream_id,
                                                stream_control: StreamControl::StepVolume(steps),
                                            })
                                            .await?;
                                    }
                                }
                            }
                            ChannelInput::SetMuted(muted) => {
                                for binding in self.bindings.neighbors_of_left(channel_id) {
                                    if let Some(stream_id) = self.binding_stream_id(&binding) {
                                        self.audio_control_tx
                                            .send(AudioControl::StreamControl {
                                                stream_id,
                                                stream_control: StreamControl::SetMuted(muted),
                                            })
                                            .await?;
                                    }
                                }
                            }
                            ChannelInput::ToggleMuted => {
                                for binding in self.bindings.neighbors_of_left(channel_id) {
                                    if let Some(stream_id) = self.binding_stream_id(&binding) {
                                        self.audio_control_tx
                                            .send(AudioControl::StreamControl {
                                                stream_id,
                                                stream_control: StreamControl::ToggleMuted,
                                            })
                                            .await?;
                                    }
                                }
                            }
                            ChannelInput::OpenMenu => {
                                self.open_menu(channel_id).await?;
                            }
                            ChannelInput::CloseMenu => {
                                self.close_menu(channel_id).await?;
                            }
                            ChannelInput::MenuNext => {
                                self.menu_next(channel_id).await?;
                            }
                            ChannelInput::MenuPrevious => {
                                self.menu_previous(channel_id).await?;
                            }
                            ChannelInput::MenuSelect => {
                                self.menu_select(channel_id).await?;
                            }
                        }
                    }
                },
                None => break,
            }
        }
        Ok(())
    }

    async fn open_menu(&mut self, channel_id: ChannelId) -> anyhow::Result<()> {
        let ChannelId(device_id, channel_index) = channel_id;

        let mut options = Vec::new();
        options.push(MenuOption {
            name: "None".into(),
            binding: None,
        });
        options.push(MenuOption {
            name: "Default Device".into(),
            binding: Some(Binding::DefaultDevice),
        });
        options.push(MenuOption {
            name: "Active Window".into(),
            binding: Some(Binding::ActiveWindow),
        });
        options.extend(
            self.streams
                .iter()
                .map(|(stream_id, stream_state)| MenuOption {
                    name: stream_state.name.clone(),
                    binding: Some(Binding::Direct(*stream_id)),
                }),
        );
        options[3..].sort_by(|a, b| a.name.cmp(&b.name));
        let menu = Menu {
            options,
            current_index: 0,
        };
        menu.print();
        self.menus.insert(channel_id, menu);
        self.control_output_tx
            .send(ControlOutput::ChannelOutput(
                device_id,
                channel_index,
                ChannelOutput::MenuOpened,
            ))
            .await?;
        Ok(())
    }

    async fn close_menu(&mut self, channel_id: ChannelId) -> anyhow::Result<()> {
        let ChannelId(device_id, channel_index) = channel_id;

        self.menus.remove(&channel_id);
        self.control_output_tx
            .send(ControlOutput::ChannelOutput(
                device_id,
                channel_index,
                ChannelOutput::MenuClosed,
            ))
            .await?;
        Ok(())
    }

    async fn menu_next(&mut self, channel_id: ChannelId) -> anyhow::Result<()> {
        if let Some(menu) = self.menus.get_mut(&channel_id) {
            menu.current_index = (menu.current_index + 1).min(menu.options.len() - 1);
            menu.print();
        }
        Ok(())
    }

    async fn menu_previous(&mut self, channel_id: ChannelId) -> anyhow::Result<()> {
        if let Some(menu) = self.menus.get_mut(&channel_id) {
            menu.current_index = menu.current_index.saturating_sub(1);
            menu.print();
        }
        Ok(())
    }

    async fn menu_select(&mut self, channel_id: ChannelId) -> anyhow::Result<()> {
        if let Some(menu) = self.menus.get(&channel_id) {
            let option = menu.options[menu.current_index].clone();
            log::info!("selected {:?}", option.name);
            self.bind(channel_id, option.binding).await?;
            self.close_menu(channel_id).await?;
        }
        Ok(())
    }

    async fn bind(
        &mut self,
        channel_id: ChannelId,
        binding: Option<Binding>,
    ) -> anyhow::Result<()> {
        self.bindings.remove_left(channel_id);
        if let Some(binding) = binding {
            self.bindings.add_edge(channel_id, binding);
        }
        self.update_channel(channel_id).await?;
        Ok(())
    }

    fn get_binding_state(&self, binding: &Binding) -> Option<&Stream> {
        self.binding_stream_id(binding)
            .and_then(|stream_id| self.streams.get(&stream_id))
    }

    fn binding_stream_id(&self, binding: &Binding) -> Option<StreamId> {
        match binding {
            Binding::Direct(stream_id) => Some(*stream_id),
            Binding::ActiveWindow => self.window_focus,
            Binding::DefaultDevice => self.default_device,
        }
    }

    async fn update_channel(&self, channel_id: ChannelId) -> anyhow::Result<()> {
        let ChannelId(device_id, channel_index) = channel_id;
        for binding in self.bindings.neighbors_of_left(channel_id) {
            let state = self
                .get_binding_state(&binding)
                .map(|stream| stream.state)
                .unwrap_or_default();
            self.control_output_tx
                .send(ControlOutput::ChannelOutput(
                    device_id,
                    channel_index,
                    ChannelOutput::StateChanged(state),
                ))
                .await?;
        }
        Ok(())
    }

    async fn update_bound_channels(&self, binding: Binding) -> anyhow::Result<()> {
        let state = self
            .get_binding_state(&binding)
            .map(|stream| stream.state)
            .unwrap_or_default();
        for ChannelId(device_id, channel_index) in self.bindings.neighbors_of_right(binding) {
            self.control_output_tx
                .send(ControlOutput::ChannelOutput(
                    device_id,
                    channel_index,
                    ChannelOutput::StateChanged(state),
                ))
                .await?;
        }
        Ok(())
    }
}

#[derive(Debug)]
enum Incoming {
    AudioEvent(AudioEvent),
    ControlInput(ControlInput),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ChannelId(DeviceId, usize);

struct Menu {
    options: Vec<MenuOption>,
    current_index: usize,
}

impl Menu {
    fn print(&self) {
        for (i, option) in self.options.iter().enumerate() {
            if i == self.current_index {
                print!("> ");
            } else {
                print!("  ");
            }
            println!("{}", option.name);
        }
        println!();
    }
}

#[derive(Clone)]
struct MenuOption {
    name: String,
    binding: Option<Binding>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Binding {
    Direct(StreamId),
    ActiveWindow,
    DefaultDevice,
}

struct Stream {
    name: String,
    state: StreamState,
}

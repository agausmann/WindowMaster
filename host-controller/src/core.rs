use crate::{
    audio::{AudioBackend, AudioControl, AudioEvent, AudioHandle, StreamControl, StreamId},
    control::{ChannelInput, ControlBackend, ControlHandle, ControlInput, ControlOutput},
};
use smol::{
    channel::{Receiver, Sender},
    future::FutureExt,
};

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
        let (audio_event_tx, audio_event_rx) = smol::channel::bounded(16);
        let (audio_control_tx, audio_control_rx) = smol::channel::bounded(16);
        let audio_handle = AudioHandle::new(audio_event_tx, audio_control_rx);
        let audio_task = self.audio_backend.start(audio_handle);

        let (control_input_tx, control_input_rx) = smol::channel::bounded(16);
        let (control_output_tx, control_output_rx) = smol::channel::bounded(16);
        let control_handle = ControlHandle::new(control_input_tx, control_output_rx);
        let control_task = self.control_backend.start(control_handle);

        let mut runtime = CoreRuntime {
            audio_event_rx,
            audio_control_tx,
            control_input_rx,
            control_output_tx,
            stream_id: None,
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

struct CoreRuntime {
    audio_event_rx: Receiver<AudioEvent>,
    audio_control_tx: Sender<AudioControl>,
    control_input_rx: Receiver<ControlInput>,
    control_output_tx: Sender<ControlOutput>,
    stream_id: Option<StreamId>,
}

impl CoreRuntime {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + 'static>> {
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
                    AudioEvent::StreamOpened { stream_id, name } => {
                        self.stream_id = Some(stream_id);
                    }
                    AudioEvent::StreamClosed { stream_id } => {}
                    AudioEvent::StreamEvent {
                        stream_id,
                        stream_event,
                    } => {}
                },
                Some(Incoming::ControlInput(control_input)) => match control_input {
                    ControlInput::ChannelInput(channel_id, channel_input) => {
                        let stream_control = match channel_input {
                            ChannelInput::SetVolume(volume) => StreamControl::SetVolume(volume),
                            ChannelInput::StepVolume(steps) => StreamControl::StepVolume(steps),
                            ChannelInput::SetMuted(muted) => StreamControl::SetMuted(muted),
                            ChannelInput::ToggleMuted => StreamControl::ToggleMuted,
                        };
                        self.audio_control_tx
                            .send(AudioControl::StreamControl {
                                stream_id: self.stream_id.unwrap(),
                                stream_control,
                            })
                            .await?;
                    }
                },
                None => break,
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
enum Incoming {
    AudioEvent(AudioEvent),
    ControlInput(ControlInput),
}

use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
};

use smol::channel::{Receiver, Sender};

pub type VolumeLevel = f32;

pub trait AudioBackend {
    type Error: std::error::Error + 'static;

    fn start(self, handle: AudioHandle) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>>>>;
}

pub struct AudioHandle {
    event_tx: Sender<AudioEvent>,
    control_rx: Receiver<AudioControl>,
}

impl AudioHandle {
    pub(crate) fn new(event_tx: Sender<AudioEvent>, control_rx: Receiver<AudioControl>) -> Self {
        Self {
            event_tx,
            control_rx,
        }
    }

    /// Create a "dummy" handle that doesn't receive any controls and discards all sent events.
    /// May be useful for testing audio backends independently.
    pub fn dummy() -> Self {
        // Event receiver can be dropped, it won't cause a hangup.
        let (event_tx, _event_rx) = smol::channel::unbounded();
        // Control sender cannot be dropped, it will cause a hangup.
        let (control_tx, control_rx) = smol::channel::unbounded();
        std::mem::forget(control_tx);

        Self {
            event_tx,
            control_rx,
        }
    }

    pub async fn send(&self, event: AudioEvent) {
        self.event_tx.send(event).await.ok();
    }

    pub async fn recv(&self) -> Option<AudioControl> {
        self.control_rx.recv().await.ok()
    }
}

#[derive(Debug)]
pub enum AudioEvent {
    StreamOpened {
        stream_id: StreamId,
        stream_info: StreamInfo,
    },
    StreamClosed {
        stream_id: StreamId,
    },
    StreamEvent {
        stream_id: StreamId,
        stream_event: StreamEvent,
    },
    WindowFocusChanged {
        stream_id: Option<StreamId>,
    },
    DefaultDeviceChanged {
        stream_id: Option<StreamId>,
    },
}

#[derive(Debug)]
pub enum StreamEvent {
    StateChanged(StreamState),
}

#[derive(Debug)]
pub enum AudioControl {
    StreamControl {
        stream_id: StreamId,
        stream_control: StreamControl,
    },
}

#[derive(Debug)]
pub enum StreamControl {
    SetVolume(VolumeLevel),
    StepVolume(i32),
    SetMuted(bool),
    ToggleMuted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamId(u64);

impl StreamId {
    /// Generates a new stream ID that is guaranteed to be unique.
    ///
    /// These IDs are only meant to be used by the running process that created
    /// them; they should not be sent to other processes or saved to disk, etc.
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id == u64::MAX {
            panic!("StreamId overflow");
        }
        Self(id)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StreamState {
    pub volume: f32,
    pub muted: bool,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            volume: 0.0,
            muted: false,
        }
    }
}

#[derive(Debug)]
pub struct StreamInfo {
    name: String,
    initial_state: StreamState,
    parent: Option<StreamId>,
}

impl StreamInfo {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn initial_state(&self) -> StreamState {
        self.initial_state
    }

    pub fn parent(&self) -> Option<StreamId> {
        self.parent
    }
}

pub struct StreamInfoBuilder {
    name: String,
    initial_state: StreamState,
    parent: Option<StreamId>,
}

impl StreamInfoBuilder {
    pub fn new(name: String) -> Self {
        Self {
            name,
            initial_state: Default::default(),
            parent: None,
        }
    }

    pub fn with_parent(self, parent: StreamId) -> Self {
        Self {
            parent: Some(parent),
            ..self
        }
    }

    pub fn with_initial_state(self, initial_state: StreamState) -> Self {
        Self {
            initial_state,
            ..self
        }
    }

    pub fn build(self) -> StreamInfo {
        StreamInfo {
            name: self.name,
            initial_state: self.initial_state,
            parent: self.parent,
        }
    }
}

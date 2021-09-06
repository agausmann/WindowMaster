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
        name: String,
        volume: f32,
        muted: bool,
    },
    StreamClosed {
        stream_id: StreamId,
    },
    StreamEvent {
        stream_id: StreamId,
        stream_event: StreamEvent,
    },
}

#[derive(Debug)]
pub enum StreamEvent {
    VolumeChanged(VolumeLevel),
    MutedChanged(bool),
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

use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
};

use smol::channel::{Receiver, Sender, TryRecvError};

use crate::audio::StreamState;

type ChannelIndex = usize;

pub trait ControlBackend {
    type Error: std::error::Error + 'static;

    fn start(self, handle: ControlHandle)
        -> Pin<Box<dyn Future<Output = Result<(), Self::Error>>>>;
}

pub struct ControlHandle {
    input_tx: Sender<ControlInput>,
    output_rx: Receiver<ControlOutput>,
}

impl ControlHandle {
    pub(crate) fn new(input_tx: Sender<ControlInput>, output_rx: Receiver<ControlOutput>) -> Self {
        Self {
            input_tx,
            output_rx,
        }
    }

    /// Create a "dummy" handle that doesn't receive any controls and discards all sent events.
    /// May be useful for testing audio backends independently.
    pub fn dummy() -> Self {
        // Input receiver can be dropped, it won't cause a hangup.
        let (input_tx, _input_rx) = smol::channel::unbounded();
        // Output sender cannot be dropped, it will cause a hangup.
        let (output_tx, output_rx) = smol::channel::unbounded();
        std::mem::forget(output_tx);

        Self {
            input_tx,
            output_rx,
        }
    }

    pub async fn send(&self, event: ControlInput) {
        self.input_tx.send(event).await.ok();
    }

    pub async fn recv(&self) -> Option<ControlOutput> {
        self.output_rx.recv().await.ok()
    }

    pub fn blocking_send(&self, event: ControlInput) {
        smol::block_on(self.input_tx.send(event)).ok();
    }

    pub fn try_recv(&self) -> Result<ControlOutput, TryRecvError> {
        self.output_rx.try_recv()
    }
}

#[derive(Debug)]
pub enum ControlInput {
    DeviceAdded(DeviceId, DeviceInfo),
    DeviceRemoved(DeviceId),
    ChannelInput(DeviceId, ChannelIndex, ChannelInput),
}

#[derive(Debug)]
pub enum ChannelInput {
    SetVolume(f32),
    StepVolume(i32),
    SetMuted(bool),
    ToggleMuted,
    OpenMenu,
    CloseMenu,
    MenuNext,
    MenuPrevious,
    MenuSelect,
}

#[derive(Debug)]
pub enum ControlOutput {
    ChannelOutput(DeviceId, ChannelIndex, ChannelOutput),
}

#[derive(Debug, Clone)]
pub enum ChannelOutput {
    StateChanged(StreamState),
    MenuOpened,
    MenuClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u64);

impl DeviceId {
    /// Generates a new stream ID that is guaranteed to be unique.
    ///
    /// These IDs are only meant to be used by the running process that created
    /// them; they should not be sent to other processes or saved to disk, etc.
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id == u64::MAX {
            panic!("DeviceId overflow");
        }
        Self(id)
    }
}

#[derive(Debug)]
pub struct DeviceInfo {
    name: String,
    num_channels: usize,
}

impl DeviceInfo {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn num_channels(&self) -> usize {
        self.num_channels
    }
}

pub struct DeviceInfoBuilder {
    name: String,
    num_channels: usize,
}

impl DeviceInfoBuilder {
    pub fn new(name: String, num_channels: usize) -> Self {
        Self { name, num_channels }
    }

    pub fn build(self) -> DeviceInfo {
        DeviceInfo {
            name: self.name,
            num_channels: self.num_channels,
        }
    }
}

use std::{future::Future, pin::Pin};

use smol::channel::{Receiver, Sender};

pub type ChannelId = u64;

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
}

#[derive(Debug)]
pub enum ControlInput {
    ChannelInput(ChannelId, ChannelInput),
}

#[derive(Debug)]
pub enum ChannelInput {
    SetVolume(f32),
    StepVolume(i32),
    SetMuted(bool),
    ToggleMuted,
}

#[derive(Debug)]
pub enum ControlOutput {}

use std::sync::mpsc::{self, TryRecvError};
use std::convert::TryFrom;

pub struct Manager {
    input_tx: mpsc::Sender<TaggedInput>,
    input_rx: mpsc::Receiver<TaggedInput>,
    channels: Vec<Channel>,
}

struct Channel {
    update_tx: mpsc::Sender<Update>,
    menu_size: usize,
    menu_index: usize,
    //TODO these states belong in an associated stream:
    volume: f32,
    muted: bool,
}

impl Channel {
    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.update_tx.send(Update::Volume(self.volume)).ok();
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.update_tx.send(Update::Mute(self.muted)).ok();
    }
    
    fn set_menu_index(&mut self, index: usize) {
        self.menu_index = index;
        self.update_tx.send(Update::MenuIndex(self.menu_index)).ok();
    }

    fn open_menu(&mut self, menu: Vec<String>, index: usize) {
        self.menu_index = index;
        self.menu_size = menu.len();
        self.update_tx.send(Update::OpenMenu(menu, index)).ok();
    }

    fn close_menu(&mut self) {
        self.menu_size = 0;
        self.update_tx.send(Update::CloseMenu).ok();
    }
}

impl Manager {
    pub fn new() -> Self {
        let (input_tx, input_rx) = mpsc::channel();
        Self {
            input_tx,
            input_rx,
            channels: Vec::new(),
        }
    }
    pub fn run(&mut self) {
        const VOLUME_INCREMENT: f32 = 0.05;

        loop {
            while let Ok(TaggedInput { channel_id, input }) = self.input_rx.try_recv() {
                println!("{} {:?}", channel_id, input);
                let channel = &mut self.channels[channel_id];
                match input {
                    Input::Volume(volume) => {
                        channel.set_volume(volume);
                    }
                    Input::VolumeStep(steps) => {
                        channel.set_volume(channel.volume + VOLUME_INCREMENT * steps as f32)
                    }
                    Input::Mute(muted) => {
                        channel.set_muted(muted);
                    }
                    Input::ToggleMute => {
                        channel.set_muted(!channel.muted);
                    }
                    Input::MenuIndex(index) => {
                        channel.set_menu_index(index);
                    }
                    Input::MenuStep(steps) => {
                            let len = i32::try_from(channel.menu_size).unwrap();
                            channel.set_menu_index(
                                usize::try_from(((channel.menu_index as i32 + steps) % len + len) % len)
                                    .unwrap(),
                            );
                    }
                    Input::Select => {
                        channel.close_menu();
                    }
                    Input::OpenMenu => {
                        channel.open_menu(vec!["Close".into()], 0);
                    }
                    Input::CloseMenu => {
                        channel.close_menu();
                    }
                }
            }
        }
    }

    pub fn register_channel(&mut self) -> Handle {
        let channel_id = self.channels.len();
        let (update_tx, update_rx) = mpsc::channel();
        let input_tx = self.input_tx.clone();
        self.channels.push(Channel { update_tx, menu_index: 0, menu_size: 0, muted: false, volume: 0.0 });
        Handle {
            channel_id,
            input_tx,
            update_rx,
        }
    }
}

#[derive(Debug)]
pub struct Handle {
    channel_id: usize,
    input_tx: mpsc::Sender<TaggedInput>,
    update_rx: mpsc::Receiver<Update>,
}

impl Handle {
    pub fn input(&self, input: Input) {
        self.input_tx
            .send(TaggedInput {
                channel_id: self.channel_id,
                input,
            })
            .unwrap()
    }

    pub fn poll_updates(&self) -> Option<Update> {
        match self.update_rx.try_recv() {
            Ok(update) => Some(update),
            Err(TryRecvError::Empty) => None,
            Err(err) => panic!("{}", err),
        }
    }
}

/// Input events from a channel to the manager.
#[derive(Debug)]
pub enum Input {
    /// Sets the stream volume (0.0..1.0).
    Volume(f32),
    /// Adjusts the stream volume by a relative step amount.
    VolumeStep(i32),
    /// Sets whether the stream is muted.
    Mute(bool),
    /// Toggles the stream mute state.
    ToggleMute,
    /// Highlights the menu option with the given index.
    MenuIndex(usize),
    /// Adjusts the highlighted menu option by a relative step amount.
    MenuStep(i32),
    /// Selects the highlighted menu option.
    Select,
    /// Explicitly opens the menu.
    OpenMenu,
    /// Explicitly closes/cancels the menu.
    CloseMenu,
}

/// State updates from the manager to a channel, so the channel can correctly
/// display the state on its interface.
#[derive(Debug)]
pub enum Update {
    /// The volume of the stream assigned to this channel (0.0..1.0).
    Volume(f32),
    /// Whether or not the stream is muted.
    Mute(bool),
    /// Opens a menu with the given list of options and initial
    /// highlight index. If one is already open, it is replaced by the given one.
    OpenMenu(Vec<String>, usize),
    /// Closes the menu if one is open.
    CloseMenu,
    /// The index of the highlighted menu option.
    MenuIndex(usize),
}

struct TaggedInput {
    channel_id: usize,
    input: Input,
}

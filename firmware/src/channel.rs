use crate::encoder::Encoder;
use crate::led::Led;
use crate::switch::Switch;
use embedded_hal::digital::v2::{InputPin, OutputPin};

pub struct Channel<A, B, S, L>
where
    A: InputPin,
    B: InputPin,
    S: InputPin,
    L: OutputPin,
{
    encoder: Encoder<A, B>,
    switch: Switch<S>,
    led: Led<L>,
}

impl<A, B, S, L> Channel<A, B, S, L>
where
    A: InputPin,
    B: InputPin,
    S: InputPin,
    L: OutputPin,
{
    pub fn encoder(&mut self) -> &mut Encoder<A, B> {
        &mut self.encoder
    }

    pub fn switch(&mut self) -> &mut Switch<S> {
        &mut self.switch
    }

    pub fn led(&mut self) -> &mut Led<L> {
        &mut self.led
    }
}

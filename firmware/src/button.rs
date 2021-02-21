use core::convert::Infallible;
use embedded_hal::digital::v2::InputPin;

/// A button that can be pressed or un-pressed.
pub trait Button {
    type Error;

    /// Polls the encoder for updates, and returns `true` if the button is currently being pressed.
    fn poll(&mut self) -> Result<bool, Self::Error>;
}

/// An active-low button.
pub struct ActiveLow<S> {
    pin: S,
}

impl<S> ActiveLow<S> {
    /// Creates a new button from the given active-low input pin.
    pub fn new(pin: S) -> Self {
        Self { pin }
    }
}

impl<S> Button for ActiveLow<S>
where
    S: InputPin,
{
    type Error = S::Error;

    fn poll(&mut self) -> Result<bool, Self::Error> {
        self.pin.is_low()
    }
}

#[derive(Default)]
pub struct Disabled;

impl Button for Disabled {
    type Error = Infallible;

    fn poll(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

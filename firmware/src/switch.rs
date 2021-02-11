use embedded_hal::digital::v2::InputPin;

/// A switch or momentary button.
pub struct Switch<S: InputPin> {
    pin: S,
    pressed: bool,
}

impl<S: InputPin> Switch<S> {
    /// Creates a new switch from the given input pin.
    pub fn new(pin: S) -> Self {
        Self {
            pin,
            pressed: false,
        }
    }

    /// True if the switch was pressed at the last poll.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Polls the pin and updates the state of the switch. Returns `true` if the switch state has
    /// changed (a positive or negative edge event).
    pub fn poll(&mut self) -> Result<bool, S::Error> {
        let new_pressed = self.pin.is_low()?;

        let edge = match (self.pressed, new_pressed) {
            (false, true) | (true, false) => true,
            _ => false,
        };

        self.pressed = new_pressed;
        Ok(edge)
    }
}

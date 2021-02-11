use embedded_hal::digital::v2::OutputPin;

/// Active-low output LED.
pub struct Led<L: OutputPin> {
    pin: L,
}

impl<L: OutputPin> Led<L> {
    /// Create a new LED from the given output pin.
    pub fn new(pin: L) -> Self {
        Self { pin }
    }

    /// Turns the LED on, by bringing the pin low.
    pub fn turn_on(&mut self) -> Result<(), L::Error> {
        self.pin.set_low()
    }

    /// Turns the LED off, by bringing the pin high.
    pub fn turn_off(&mut self) -> Result<(), L::Error> {
        self.pin.set_high()
    }
}

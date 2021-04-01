use core::convert::Infallible;
use embedded_hal::digital::v2::OutputPin;

/// An indicator output that can be switched on or off.
pub trait Indicator {
    type Error;

    /// Switches the indicator on.
    fn turn_on(&mut self) -> Result<(), Self::Error>;

    /// Switches the indicator off.
    fn turn_off(&mut self) -> Result<(), Self::Error>;
}

/// Active-low output LED.
pub struct ActiveLow<L> {
    pin: L,
}

impl<L> ActiveLow<L>
where
    L: OutputPin,
{
    /// Create a new LED from the given output pin.
    pub fn new(mut pin: L) -> Self {
        pin.set_high().ok();
        Self { pin }
    }
}

impl<L> Indicator for ActiveLow<L>
where
    L: OutputPin,
{
    type Error = L::Error;

    fn turn_on(&mut self) -> Result<(), Self::Error> {
        self.pin.set_low()
    }

    fn turn_off(&mut self) -> Result<(), Self::Error> {
        self.pin.set_high()
    }
}

/// Active-high output LED.
pub struct ActiveHigh<L> {
    pin: L,
}

impl<L> ActiveHigh<L>
where
    L: OutputPin,
{
    /// Create a new LED from the given output pin.
    pub fn new(mut pin: L) -> Self {
        pin.set_low().ok();
        Self { pin }
    }
}

impl<L> Indicator for ActiveHigh<L>
where
    L: OutputPin,
{
    type Error = L::Error;

    fn turn_on(&mut self) -> Result<(), Self::Error> {
        self.pin.set_high()
    }

    fn turn_off(&mut self) -> Result<(), Self::Error> {
        self.pin.set_low()
    }
}

#[derive(Default)]
pub struct Disabled;

impl Indicator for Disabled {
    type Error = Infallible;

    fn turn_on(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn turn_off(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

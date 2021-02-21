use core::convert::Infallible;
use embedded_hal::digital::v2::InputPin;

/// A relative, incremental encoder that can detect and report single steps forward and backward.
pub trait Encoder {
    type Error;

    /// Polls the encoder for updates, and returns the resulting step, if any.
    fn poll(&mut self) -> Result<Step, Self::Error>;
}

/// A step increment returned by an [`Encoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    None,
    Backward,
    Forward,
}

impl Step {
    pub fn value(self) -> i8 {
        match self {
            Self::None => 0,
            Self::Backward => -1,
            Self::Forward => 1,
        }
    }
}

/// A quadrature encoder peripheral.
pub struct Quadrature<A, B> {
    pin_a: A,
    pin_b: B,
    old_index: Option<i8>,
}

impl<A, B> Quadrature<A, B> {
    /// Creates a new encoder from the given input pins.
    pub fn new(pin_a: A, pin_b: B) -> Self {
        Self {
            pin_a,
            pin_b,
            old_index: None,
        }
    }
}

impl<A, B> Encoder for Quadrature<A, B>
where
    A: InputPin,
    B: InputPin,
{
    type Error = Error<A::Error, B::Error>;

    fn poll(&mut self) -> Result<Step, Self::Error> {
        let a = self.pin_a.is_high().map_err(Error::PinA)?;
        let b = self.pin_b.is_high().map_err(Error::PinB)?;
        let new_index = index(a, b);

        let result = if let Some(old_index) = self.old_index {
            match (new_index + (4 - old_index)) % 4 {
                0 => Ok(Step::None),
                1 => Ok(Step::Forward),
                2 => Err(Error::Skipped),
                3 => Ok(Step::Backward),
                _ => unreachable!(),
            }
        } else {
            Ok(Step::None)
        };
        self.old_index = Some(new_index);
        result
    }
}

fn index(a: bool, b: bool) -> i8 {
    match (a, b) {
        (false, false) => 0,
        (true, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    }
}

#[derive(Debug)]
pub enum Error<A, B> {
    /// An error that occurred while reading Pin A.
    PinA(A),

    /// An error that occurred while reading Pin B.
    PinB(B),

    /// A skipped step was detected.
    Skipped,
}

#[derive(Default)]
pub struct Disabled;

impl Encoder for Disabled {
    type Error = Infallible;

    fn poll(&mut self) -> Result<Step, Self::Error> {
        Ok(Step::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::pin;
    use embedded_hal_mock::pin::Transaction;
    use embedded_hal_mock::{High, Low};
    use Step::{Backward, Forward, None};

    fn assert_polls(pin_a: &[pin::Transaction], pin_b: &[pin::Transaction], polls: &[Step]) {
        let mut pin_a = pin::Mock::new(pin_a);
        let mut pin_b = pin::Mock::new(pin_b);
        let mut encoder = Encoder::new(pin_a.clone(), pin_b.clone());
        for &value in polls {
            assert_eq!(encoder.poll().unwrap(), value);
        }
        pin_a.done();
        pin_b.done();
    }

    #[test]
    fn forward() {
        assert_polls(
            &[
                Transaction::get(Low),
                Transaction::get(High),
                Transaction::get(High),
                Transaction::get(Low),
                Transaction::get(Low),
            ],
            &[
                Transaction::get(Low),
                Transaction::get(Low),
                Transaction::get(High),
                Transaction::get(High),
                Transaction::get(Low),
            ],
            &[None, Forward, Forward, Forward, Forward],
        );
    }

    #[test]
    fn backward() {
        assert_polls(
            &[
                Transaction::get(Low),
                Transaction::get(Low),
                Transaction::get(High),
                Transaction::get(High),
                Transaction::get(Low),
            ],
            &[
                Transaction::get(Low),
                Transaction::get(High),
                Transaction::get(High),
                Transaction::get(Low),
                Transaction::get(Low),
            ],
            &[None, Backward, Backward, Backward, Backward],
        );
    }

    #[test]
    #[should_panic]
    fn skipped() {
        assert_polls(
            &[Transaction::get(Low), Transaction::get(High)],
            &[Transaction::get(Low), Transaction::get(High)],
            &[None, Forward],
        );
    }
}

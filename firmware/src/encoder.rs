use embedded_hal::digital::v2::InputPin;

/// A quadrature encoder peripheral.
pub struct Encoder<A: InputPin, B: InputPin> {
    pin_a: A,
    pin_b: B,
    old_index: Option<i8>,
}

impl<A: InputPin, B: InputPin> Encoder<A, B> {
    /// Creates a new encoder from the given input pins.
    pub fn new(pin_a: A, pin_b: B) -> Self {
        Self {
            pin_a,
            pin_b,
            old_index: None,
        }
    }

    /// Reads the input pins and updates the encoder state.
    ///
    /// Returns an offset corresponding to the number of steps traveled (-1, 0, or 1), or an error
    /// if the input pins failed or if a step was skipped.
    pub fn poll(&mut self) -> Result<Delta, PollError<A::Error, B::Error>> {
        let a = self.pin_a.is_high().map_err(PollError::PinA)?;
        let b = self.pin_b.is_high().map_err(PollError::PinB)?;
        let new_index = index(a, b);

        let result = if let Some(old_index) = self.old_index {
            match (new_index + (4 - old_index)) % 4 {
                0 => Ok(Delta::None),
                1 => Ok(Delta::Clockwise),
                2 => Err(PollError::Skipped),
                3 => Ok(Delta::Counterclockwise),
                _ => unreachable!(),
            }
        } else {
            Ok(Delta::None)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delta {
    Clockwise,
    Counterclockwise,
    None,
}

#[derive(Debug)]
pub enum PollError<A, B> {
    /// An error that occurred while reading Pin A.
    PinA(A),

    /// An error that occurred while reading Pin B.
    PinB(B),

    /// A skipped step was detected.
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::pin;

    fn assert_polls(pin_a: &[pin::Transaction], pin_b: &[pin::Transaction], polls: &[i8]) {
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
    fn clockwise() {
        assert_polls(
            &[
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::High),
                pin::Transaction::get(pin::State::High),
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::Low),
            ],
            &[
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::High),
                pin::Transaction::get(pin::State::High),
                pin::Transaction::get(pin::State::Low),
            ],
            &[0, 1, 1, 1, 1],
        );
    }

    #[test]
    fn counter_clockwise() {
        assert_polls(
            &[
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::High),
                pin::Transaction::get(pin::State::High),
                pin::Transaction::get(pin::State::Low),
            ],
            &[
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::High),
                pin::Transaction::get(pin::State::High),
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::Low),
            ],
            &[0, -1, -1, -1, -1],
        );
    }

    #[test]
    #[should_panic]
    fn skipped() {
        assert_polls(
            &[
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::High),
            ],
            &[
                pin::Transaction::get(pin::State::Low),
                pin::Transaction::get(pin::State::High),
            ],
            &[0, 1],
        );
    }
}

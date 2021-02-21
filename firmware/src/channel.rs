use crate::button;
use crate::encoder;
use crate::indicator;

pub trait Channel {
    type Encoder: encoder::Encoder;
    type Button: button::Button;
    type Indicator: indicator::Indicator;

    fn encoder(&mut self) -> &mut Self::Encoder;

    fn button(&mut self) -> &mut Self::Button;

    fn indicator(&mut self) -> &mut Self::Indicator;
}

pub struct ChannelImpl<Encoder, Button, Indicator> {
    encoder: Encoder,
    button: Button,
    indicator: Indicator,
}

impl<Encoder, Button, Indicator> ChannelImpl<Encoder, Button, Indicator> {
    pub fn new(encoder: Encoder, button: Button, indicator: Indicator) -> Self {
        Self {
            encoder,
            button,
            indicator,
        }
    }
}

impl<Encoder, Button, Indicator> Channel for ChannelImpl<Encoder, Button, Indicator>
where
    Encoder: encoder::Encoder,
    Button: button::Button,
    Indicator: indicator::Indicator,
{
    type Encoder = Encoder;
    type Button = Button;
    type Indicator = Indicator;

    fn encoder(&mut self) -> &mut Self::Encoder {
        &mut self.encoder
    }

    fn button(&mut self) -> &mut Self::Button {
        &mut self.button
    }

    fn indicator(&mut self) -> &mut Self::Indicator {
        &mut self.indicator
    }
}

#[derive(Default)]
pub struct Disabled {
    encoder: encoder::Disabled,
    button: button::Disabled,
    indicator: indicator::Disabled,
}

impl Channel for Disabled {
    type Encoder = encoder::Disabled;
    type Button = button::Disabled;
    type Indicator = indicator::Disabled;

    fn encoder(&mut self) -> &mut Self::Encoder {
        &mut self.encoder
    }

    fn button(&mut self) -> &mut Self::Button {
        &mut self.button
    }

    fn indicator(&mut self) -> &mut Self::Indicator {
        &mut self.indicator
    }
}

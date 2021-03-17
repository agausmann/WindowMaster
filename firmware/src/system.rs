use crate::button::{self, Button};
use crate::channel::{self, Channel, ChannelImpl};
use crate::encoder::{self, Encoder};
use crate::indicator::{self, Indicator};
use crate::link::{self, Link};
use alloc::boxed::Box;
use cortex_m::interrupt::CriticalSection;
use stm32f0xx_hal::delay::Delay;
use stm32f0xx_hal::gpio::{gpioa, gpiob, gpioc, gpiof, Floating, Input, Output, PullUp, PushPull};
use stm32f0xx_hal::prelude::*;
use stm32f0xx_hal::stm32::{CorePeripherals, Peripherals};
use stm32f0xx_hal::usb::{self, UsbBus};

pub struct System<StatusLed, Channel1, Channel2, Channel3, Channel4, Channel5, Channel6, HostLink> {
    status_led: StatusLed,
    delay: Delay,
    channel_1: Channel1,
    channel_2: Channel2,
    channel_3: Channel3,
    channel_4: Channel4,
    channel_5: Channel5,
    channel_6: Channel6,
    host_link: HostLink,
}

impl<StatusLed, Channel1, Channel2, Channel3, Channel4, Channel5, Channel6, HostLink>
    System<StatusLed, Channel1, Channel2, Channel3, Channel4, Channel5, Channel6, HostLink>
{
    pub fn from_parts(
        status_led: StatusLed,
        delay: Delay,
        channel_1: Channel1,
        channel_2: Channel2,
        channel_3: Channel3,
        channel_4: Channel4,
        channel_5: Channel5,
        channel_6: Channel6,
        host_link: HostLink,
    ) -> Self {
        Self {
            status_led,
            delay,
            channel_1,
            channel_2,
            channel_3,
            channel_4,
            channel_5,
            channel_6,
            host_link,
        }
    }
}

/// A system for the WindowMaster Revision 1 hardware.
pub type Rev1System = System<
    indicator::ActiveLow<gpiob::PB12<Output<PushPull>>>,
    // Channel 1
    ChannelImpl<
        encoder::Quadrature<gpioc::PC14<Input<Floating>>, gpioc::PC13<Input<Floating>>>,
        button::ActiveLow<gpiob::PB3<Input<PullUp>>>,
        indicator::ActiveLow<gpiob::PB4<Output<PushPull>>>,
    >,
    // Channel 2
    ChannelImpl<
        encoder::Quadrature<gpiob::PB9<Input<Floating>>, gpiob::PB8<Input<Floating>>>,
        button::ActiveLow<gpioa::PA15<Input<PullUp>>>,
        indicator::ActiveLow<gpiob::PB5<Output<PushPull>>>,
    >,
    // Channel 3
    ChannelImpl<
        encoder::Quadrature<gpiob::PB7<Input<Floating>>, gpiob::PB6<Input<Floating>>>,
        button::ActiveLow<gpioa::PA14<Input<PullUp>>>,
        indicator::ActiveLow<gpioa::PA13<Output<PushPull>>>,
    >,
    // Channel 4
    ChannelImpl<
        encoder::Quadrature<gpiob::PB0<Input<Floating>>, gpioa::PA7<Input<Floating>>>,
        button::ActiveLow<gpiof::PF1<Input<PullUp>>>,
        indicator::ActiveLow<gpioa::PA0<Output<PushPull>>>,
    >,
    // Channel 5
    ChannelImpl<
        encoder::Quadrature<gpioa::PA6<Input<Floating>>, gpioa::PA5<Input<Floating>>>,
        button::ActiveLow<gpiof::PF0<Input<PullUp>>>,
        indicator::ActiveLow<gpioa::PA1<Output<PushPull>>>,
    >,
    // Channel 6
    ChannelImpl<
        encoder::Quadrature<gpioa::PA4<Input<Floating>>, gpioa::PA3<Input<Floating>>>,
        button::ActiveLow<gpioc::PC15<Input<PullUp>>>,
        indicator::ActiveLow<gpioa::PA2<Output<PushPull>>>,
    >,
    link::UsbHid<'static, UsbBus<usb::Peripheral>>,
>;

impl Rev1System {
    pub fn new(mut dp: Peripherals, cp: CorePeripherals, cs: &CriticalSection) -> Self {
        let mut rcc = dp
            .RCC
            .configure()
            .hsi48()
            .enable_crs(dp.CRS)
            .sysclk(48.mhz())
            .pclk(24.mhz())
            .freeze(&mut dp.FLASH);

        let gpioa = dp.GPIOA.split(&mut rcc);
        let gpiob = dp.GPIOB.split(&mut rcc);
        let gpioc = dp.GPIOC.split(&mut rcc);
        let gpiof = dp.GPIOF.split(&mut rcc);

        let status_led = indicator::ActiveLow::new(gpiob.pb12.into_push_pull_output(cs));
        let delay = Delay::new(cp.SYST, &rcc);

        let channel_1 = ChannelImpl::new(
            encoder::Quadrature::new(gpioc.pc14, gpioc.pc13),
            button::ActiveLow::new(gpiob.pb3.into_pull_up_input(cs)),
            indicator::ActiveLow::new(gpiob.pb4.into_push_pull_output(cs)),
        );
        let channel_2 = ChannelImpl::new(
            encoder::Quadrature::new(gpiob.pb9, gpiob.pb8),
            button::ActiveLow::new(gpioa.pa15.into_pull_up_input(cs)),
            indicator::ActiveLow::new(gpiob.pb5.into_push_pull_output(cs)),
        );
        let channel_3 = ChannelImpl::new(
            encoder::Quadrature::new(gpiob.pb7, gpiob.pb6),
            button::ActiveLow::new(gpioa.pb14.into_pull_up_input(cs)),
            indicator::ActiveLow::new(gpioa.pb15.into_push_pull_output(cs)),
        );
        let channel_4 = ChannelImpl::new(
            encoder::Quadrature::new(gpiob.pb0, gpioa.pa7),
            button::ActiveLow::new(gpiof.pf1.into_pull_up_input(cs)),
            indicator::ActiveLow::new(gpioa.pa0.into_push_pull_output(cs)),
        );
        let channel_5 = ChannelImpl::new(
            encoder::Quadrature::new(gpioa.pa6, gpioa.pa5),
            button::ActiveLow::new(gpiof.pf0.into_pull_up_input(cs)),
            indicator::ActiveLow::new(gpioa.pa1.into_push_pull_output(cs)),
        );
        let channel_6 = ChannelImpl::new(
            encoder::Quadrature::new(gpioa.pa4, gpioa.pa3),
            button::ActiveLow::new(gpioc.pc15.into_pull_up_input(cs)),
            indicator::ActiveLow::new(gpioa.pa2.into_push_pull_output(cs)),
        );

        let bus_allocator = Box::new(UsbBus::new(usb::Peripheral {
            usb: dp.USB,
            pin_dm: gpioa.pa11,
            pin_dp: gpioa.pa12,
        }));
        let host_link = link::UsbHid::new(Box::leak(bus_allocator));

        System {
            status_led,
            delay,
            channel_1,
            channel_2,
            channel_3,
            channel_4,
            channel_5,
            channel_6,
            host_link,
        }
    }
}

/// A system that is compatible with the 32F072BDISCOVERY development board.
pub type DiscoverySystem = System<
    indicator::ActiveHigh<gpioc::PC8<Output<PushPull>>>,
    ChannelImpl<
        encoder::Quadrature<gpioa::PA5<Input<Floating>>, gpioa::PA4<Input<Floating>>>,
        button::ActiveLow<gpioc::PC4<Input<Floating>>>,
        indicator::ActiveLow<gpiob::PB12<Output<PushPull>>>,
    >,
    channel::Disabled,
    channel::Disabled,
    channel::Disabled,
    channel::Disabled,
    channel::Disabled,
    link::UsbHid<'static, UsbBus<usb::Peripheral>>,
>;

impl DiscoverySystem {
    pub fn new(mut dp: Peripherals, cp: CorePeripherals, cs: &CriticalSection) -> Self {
        let mut rcc = dp
            .RCC
            .configure()
            .hsi48()
            .enable_crs(dp.CRS)
            .sysclk(48.mhz())
            .pclk(24.mhz())
            .freeze(&mut dp.FLASH);

        let gpioa = dp.GPIOA.split(&mut rcc);
        let gpiob = dp.GPIOB.split(&mut rcc);
        let gpioc = dp.GPIOC.split(&mut rcc);

        let status_led = indicator::ActiveHigh::new(gpioc.pc8.into_push_pull_output(cs));
        let delay = Delay::new(cp.SYST, &rcc);

        let channel_1 = ChannelImpl::new(
            encoder::Quadrature::new(gpioa.pa5, gpioa.pa4),
            button::ActiveLow::new(gpioc.pc4.into_floating_input(cs)),
            indicator::ActiveLow::new(gpiob.pb12.into_push_pull_output(cs)),
        );
        let channel_2 = channel::Disabled::default();
        let channel_3 = channel::Disabled::default();
        let channel_4 = channel::Disabled::default();
        let channel_5 = channel::Disabled::default();
        let channel_6 = channel::Disabled::default();

        let bus_allocator = Box::new(UsbBus::new(usb::Peripheral {
            usb: dp.USB,
            pin_dm: gpioa.pa11,
            pin_dp: gpioa.pa12,
        }));
        let host_link = link::UsbHid::new(Box::leak(bus_allocator));

        System {
            status_led,
            delay,
            channel_1,
            channel_2,
            channel_3,
            channel_4,
            channel_5,
            channel_6,
            host_link,
        }
    }
}

impl<StatusLed, Channel1, Channel2, Channel3, Channel4, Channel5, Channel6, HostLink>
    System<StatusLed, Channel1, Channel2, Channel3, Channel4, Channel5, Channel6, HostLink>
where
    StatusLed: Indicator,
    Channel1: Channel,
    Channel2: Channel,
    Channel3: Channel,
    Channel4: Channel,
    Channel5: Channel,
    Channel6: Channel,
    HostLink: Link,
{
    pub fn status_led(&mut self) -> &mut StatusLed {
        &mut self.status_led
    }

    pub fn delay(&mut self) -> &mut Delay {
        &mut self.delay
    }

    pub fn run(&mut self) -> ! {
        fn update_channel<HostLink, ChannelX>(
            host_link: &mut HostLink,
            index: usize,
            channel: &mut ChannelX,
        ) where
            HostLink: Link,
            ChannelX: Channel,
        {
            if let Ok(step) = channel.encoder().poll() {
                host_link.update_encoder(index, step);
            }
            if let Ok(is_pressed) = channel.button().poll() {
                host_link.update_button(index, is_pressed);
            }
            if host_link.is_led_on(index) {
                channel.indicator().turn_on().ok();
            } else {
                channel.indicator().turn_off().ok();
            }
        }

        self.status_led.turn_on().ok();

        loop {
            update_channel(&mut self.host_link, 0, &mut self.channel_1);
            update_channel(&mut self.host_link, 1, &mut self.channel_2);
            update_channel(&mut self.host_link, 2, &mut self.channel_3);
            update_channel(&mut self.host_link, 3, &mut self.channel_4);
            update_channel(&mut self.host_link, 4, &mut self.channel_5);
            update_channel(&mut self.host_link, 5, &mut self.channel_6);
            self.host_link.poll().ok();
        }
    }
}

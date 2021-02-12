#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate panic_semihosting;
use core::sync::atomic::AtomicBool;
use cortex_m::interrupt::free as interrupt_free;
use cortex_m_rt as rt;
use stm32_usbd::UsbBus;
use stm32f0xx_hal::prelude::*;
use stm32f0xx_hal::{stm32, usb};
use usb_device::class_prelude::*;
use usb_device::prelude::*;
use usbd_hid::descriptor::*;
use usbd_hid::hid_class::HIDClass;
use windowmaster_firmware::{Channel, Encoder, Led, Switch};

#[gen_hid_descriptor(
    (report_id = 0x01,) = {
        #[item_settings relative] encoder_1=input;
        #[item_settings relative] encoder_2=input;
        #[item_settings relative] encoder_3=input;
        #[item_settings relative] encoder_4=input;
        #[item_settings relative] encoder_5=input;
        #[item_settings relative] encoder_6=input;
        #[packed_bits 6] buttons=input;
        #[packed_bits 6] leds=output;
    }
)]
struct Report {
    encoder_1: i8,
    encoder_2: i8,
    encoder_3: i8,
    encoder_4: i8,
    encoder_5: i8,
    encoder_6: i8,
    buttons: u8,
    leds: u8,
}

// Channel I/Os have identical requirements for pin configurations.
//
// This is not possible with a function, as the pin types are different between different channels,
// and there currently is no "generic" way to set pin configurations across different pin types.
macro_rules! build_channel {
    (enc_a: $enc_a:expr, enc_b: $enc_b:expr, sw: $sw:expr, ind: $ind:expr, $guard:expr $(,)?) => {
        Channel::new(
            Encoder::new(
                $enc_a.into_floating_input($guard),
                $enc_b.into_floating_input($guard),
            ),
            Switch::new($sw.into_pull_up_input($guard)),
            Led::new($ind.into_push_pull_output($guard)),
        )
    };
}

#[cfg(not(test))]
#[rt::entry]
fn main() -> ! {
    let (channel_1, channel_2, channel_3, channel_4, channel_5, channel_6, usb_bus) =
        interrupt_free(|guard| {
            let mut dp = stm32::Peripherals::take().unwrap();

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

            let channel_1 = build_channel! {
                enc_a: gpioc.pc14,
                enc_b: gpioc.pc13,
                sw: gpiob.pb3,
                ind: gpiob.pb4,
                guard,
            };
            let channel_2 = build_channel! {
                enc_a: gpiob.pb9,
                enc_b: gpiob.pb8,
                sw: gpioa.pa15,
                ind: gpiob.pb5,
                guard,
            };
            let channel_3 = build_channel! {
                enc_a: gpiob.pb7,
                enc_b: gpiob.pb6,
                sw: gpioa.pa14,
                ind: gpioa.pa13,
                guard,
            };
            let channel_4 = build_channel! {
                enc_a: gpiob.pb0,
                enc_b: gpioa.pa7,
                sw: gpiof.pf1,
                ind: gpioa.pa0,
                guard,
            };
            let channel_5 = build_channel! {
                enc_a: gpioa.pa6,
                enc_b: gpioa.pa5,
                sw: gpiof.pf0,
                ind: gpioa.pa1,
                guard,
            };
            let channel_6 = build_channel! {
                enc_a: gpioa.pa4,
                enc_b: gpioa.pa3,
                sw: gpioc.pc15,
                ind: gpioa.pa2,
                guard,
            };

            let usb_bus = UsbBus::new(usb::Peripheral {
                usb: dp.USB,
                pin_dm: gpioa.pa11,
                pin_dp: gpioa.pa12,
            });

            (
                channel_1, channel_2, channel_3, channel_4, channel_5, channel_6, usb_bus,
            )
        });

    let mut hid = HIDClass::new(&usb_bus, Report::desc(), 10);

    //TODO Register PID.
    // Pending: 1209:4573 - https://github.com/pidcodes/pidcodes.github.com/pull/616
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x1209, 0x0001))
        .manufacturer("Adam Gausmann")
        .product("WindowMaster")
        .serial_number("NA") //TODO
        .device_class(0xEF) //TODO
        .build();

    loop {}
}

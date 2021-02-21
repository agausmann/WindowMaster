use crate::encoder::Step;
use core::convert::Infallible;
use usb_device::bus::{UsbBus, UsbBusAllocator};
use usb_device::prelude::*;
use usbd_hid::descriptor::generator_prelude::*;
use usbd_hid::hid_class::HIDClass;

/// A communication link to the host computer, to send and receive controller state.
pub trait Link {
    type Error;

    fn poll(&mut self) -> Result<(), Self::Error>;

    fn update_encoder(&mut self, index: usize, step: Step);

    fn update_button(&mut self, index: usize, is_pressed: bool);

    fn is_led_on(&self, index: usize) -> bool;
}

/// A host link implemented as a custom USB HID device.
pub struct UsbHid<'a, Bus>
where
    Bus: UsbBus,
{
    hid: HIDClass<'a, Bus>,
    device: UsbDevice<'a, Bus>,
    report: Report,
}

impl<'a, Bus> UsbHid<'a, Bus>
where
    Bus: UsbBus,
{
    pub fn new(bus_allocator: &'a UsbBusAllocator<Bus>) -> Self {
        let hid = HIDClass::new(bus_allocator, Report::desc(), 10);

        let device = UsbDeviceBuilder::new(bus_allocator, UsbVidPid(0x1209, 0x4573))
            .manufacturer("Adam Gausmann")
            .product("WindowMaster")
            .build();

        Self {
            hid,
            device,
            report: Default::default(),
        }
    }
}

impl<Bus> Link for UsbHid<'_, Bus>
where
    Bus: UsbBus,
{
    type Error = Infallible;

    fn poll(&mut self) -> Result<(), Self::Error> {
        if self.device.poll(&mut [&mut self.hid]) {
            if self.hid.push_input(&self.report).is_ok() {
                self.report.reset();
            }

            let mut buffer = [0u8; 1];
            if let Ok(read_bytes) = self.hid.pull_raw_output(&mut buffer) {
                if read_bytes > 0 {
                    //TODO leds
                }
            }
        }
        Ok(())
    }

    fn update_encoder(&mut self, index: usize, step: Step) {
        self.report.encoders[index] += step.value();
    }

    fn update_button(&mut self, index: usize, is_pressed: bool) {
        if is_pressed {
            self.report.buttons |= 1 << index;
        } else {
            self.report.buttons &= !(1 << index);
        }
    }

    fn is_led_on(&self, index: usize) -> bool {
        (self.report.leds & (1 << index)) != 0
    }
}

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = VENDOR_DEFINED_START, usage = 0x01) = {
        (usage_page = VENDOR_DEFINED_START, usage = 0x02) = {
            #[item_settings data,variable,relative] encoders=input;
        };
        (usage_page = VENDOR_DEFINED_START, usage = 0x02) = {
            #[packed_bits 6] #[item_settings data,variable,absolute] buttons=input;
        };
        (usage_page = VENDOR_DEFINED_START, usage = 0x02) = {
            #[packed_bits 6] #[item_settings data,variable,absolute] leds=output;
        };
    }
)]
#[derive(Default)]
struct Report {
    encoders: [i8; 6],
    buttons: u8,
    leds: u8,
}

impl Report {
    /// Resets all relative inputs to zero.
    fn reset(&mut self) {
        for encoder in &mut self.encoders {
            *encoder = 0;
        }
    }
}

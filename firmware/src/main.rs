#![no_std]
#![cfg_attr(not(test), no_main)]
#![feature(default_alloc_error_handler)]

use alloc_cortex_m::CortexMHeap;
use core::panic::PanicInfo;
use cortex_m::interrupt::free as interrupt_free;
use cortex_m::prelude::*;
use cortex_m_rt as rt;
use stm32f0xx_hal::stm32;
use windowmaster_firmware::indicator::Indicator;
use windowmaster_firmware::system;

// Pick a system definition here:

type System = system::DiscoverySystem;
//type System = system::Rev1System;

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // We are in an unrecoverable error state.
    // The regular program is not going to resume, so it is safe to take control of peripherals.
    // There might be problems here, since some of the peripherals may not be in their
    // reset state, like the PAC might assume, but I _think_ it's okay as long as the pin modes are
    // explicitly set somewhere before being used.
    let dp = unsafe { stm32::Peripherals::steal() };
    let cp = unsafe { stm32::CorePeripherals::steal() };

    let mut system = interrupt_free(|cs| System::new(dp, cp, cs));

    system.status_led().turn_off().ok();
    for _ in 0..4 {
        system.delay().delay_ms(200u32);
        system.status_led().turn_on().ok();
        system.delay().delay_ms(200u32);
        system.status_led().turn_off().ok();
    }

    //TODO log error via debugger.
    // writing via cortex_m_semihosting tends to hang in my testing - need to look into that.

    // Error has been logged, reset the system.
    system.delay().delay_ms(1000u32);
    stm32::SCB::sys_reset()
}

#[cfg(not(test))]
#[rt::entry]
fn main() -> ! {
    let heap_start = cortex_m_rt::heap_start() as usize;
    let heap_size = 1024;
    unsafe { ALLOCATOR.init(heap_start, heap_size) }

    let dp = stm32::Peripherals::take().unwrap();
    let cp = stm32::CorePeripherals::take().unwrap();

    let mut system = interrupt_free(|cs| System::new(dp, cp, cs));
    system.run();
}

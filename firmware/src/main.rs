#![no_std]
#![cfg_attr(not(test), no_main)]
#![feature(default_alloc_error_handler)]

//TODO custom panic handler
extern crate panic_semihosting;

use alloc_cortex_m::CortexMHeap;
use cortex_m::interrupt::free as interrupt_free;
use cortex_m_rt as rt;
use stm32f0xx_hal::stm32;
use windowmaster_firmware::system;

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

#[cfg(not(test))]
#[rt::entry]
fn main() -> ! {
    let heap_start = cortex_m_rt::heap_start() as usize;
    let heap_size = 1024;
    unsafe { ALLOCATOR.init(heap_start, heap_size) }

    let dp = stm32::Peripherals::take().unwrap();
    //let cp = stm32::CorePeripherals::take().unwrap();

    //let system = interrupt_free(|cs| system::Rev1System::new(dp, cs));
    let mut system = interrupt_free(|cs| system::DiscoverySystem::new(dp, cs));
    system.run();
}

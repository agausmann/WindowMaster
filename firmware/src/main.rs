#![no_std]
#![cfg_attr(not(test), no_main)]

extern crate panic_semihosting;
use cortex_m_rt as rt;

#[cfg(not(test))]
#[rt::entry]
fn main() -> ! {
    panic!("Hello world");
}

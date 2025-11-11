//! DaedalusOS kernel binary entry point.
//!
//! This module contains the bare-metal executable entry point and panic handlers
//! for both normal operation and test mode.

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(daedalus::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use daedalus::println;

/// Rust entry point called from boot.s
// SAFETY: no_mangle required because this is the entry point called by name from boot.s after CPU initialization.
// extern "C" ensures stable ABI. Assembly caller (boot.s) guarantees: stack is 16-byte aligned, BSS is zeroed,
// other cores are parked, MMU is disabled, running at EL1 or EL2, interrupts are masked.
#[unsafe(no_mangle)]
pub extern "C" fn _start_rust() -> ! {
    daedalus::init();

    #[cfg(test)]
    test_main();

    #[cfg(not(test))]
    {
        daedalus::shell::run();
    }

    #[cfg(test)]
    loop {}
}

/// Panic handler for normal (non-test) operation.
///
/// Prints panic information and halts the CPU indefinitely.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

/// Panic handler for test mode.
///
/// Delegates to the shared test panic handler in the library.
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    daedalus::test_panic_handler(info)
}

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

pub mod drivers;
pub mod qemu;

use core::fmt::{self, Write};

#[cfg(test)]
use core::panic::PanicInfo;

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[failed]\n");
    println!("Error: {}\n", info);
    qemu::exit(qemu::ExitCode::Failed);
}

/// Initialize the kernel
///
/// Sets up hardware devices and prepares the system for operation.
pub fn init() {
    drivers::uart::WRITER.lock().init();
}

/// Print implementation that acquires the UART writer lock
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    drivers::uart::WRITER
        .lock()
        .write_fmt(args)
        .expect("Printing to UART failed");
}

/// Print macro for console output
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

/// Println macro for console output
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// Test infrastructure
pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("{}...\t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    qemu::exit(qemu::ExitCode::Success);
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

#[test_case]
fn test_println() {
    println!("test_println output");
}

#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start_rust() -> ! {
    init();
    test_main();
    loop {}
}

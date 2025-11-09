#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(daedalus::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use daedalus::println;

#[cfg(test)]
use daedalus::qemu;

/// Rust entry point called from boot.s
#[unsafe(no_mangle)]
pub extern "C" fn _start_rust() -> ! {
    daedalus::init();

    #[cfg(test)]
    test_main();

    #[cfg(not(test))]
    {
        println!("Welcome to Daedalus OS!");
    }

    loop {}
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[failed]\n");
    println!("Error: {}\n", info);
    qemu::exit(qemu::ExitCode::Failed);
}

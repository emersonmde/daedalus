#![no_std]
#![no_main]

mod vga_driver;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("{} to {}!", "Welcome", "Daedalus");

    loop {}
}

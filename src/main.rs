#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod pl011;

use core::panic::PanicInfo;

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
    exit_qemu(QemuExitCode::Failed);
}

/// Rust entry point called from boot.s
#[unsafe(no_mangle)]
pub extern "C" fn _start_rust() -> ! {
    // Initialize the console
    pl011::CONSOLE.lock().init();

    #[cfg(test)]
    test_main();

    #[cfg(not(test))]
    {
        // Print welcome message
        println!("Welcome to Daedalus (Pi)!");
    }

    loop {}
}

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

#[cfg(test)]
fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

/// Exit QEMU using ARM semihosting
pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use core::arch::asm;

    // ARM semihosting exit
    // r0 = 0x18 (SYS_EXIT)
    // r1 = exit code
    unsafe {
        asm!(
            "mov w0, #0x18",
            "mov w1, {0:w}",
            "hlt #0xf000",
            in(reg) exit_code as u32,
            options(noreturn)
        );
    }
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

#[test_case]
fn test_println() {
    println!("test_println output");
}

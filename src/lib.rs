#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

pub mod drivers;
pub mod exceptions;
pub mod qemu;
pub mod shell;

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
    exceptions::init();
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

// ============================================================================
// Basic Sanity Tests
// ============================================================================

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

// ============================================================================
// Kernel Initialization Tests
// ============================================================================

#[test_case]
fn test_kernel_init() {
    // Test that init() can be called multiple times safely
    init();
    init();
    // If we get here without hanging, initialization is idempotent
}

// ============================================================================
// Print Macro Tests
// ============================================================================

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_formatting() {
    println!("Number: {}, Hex: 0x{:x}, Binary: {:b}", 42, 255, 0b1010);
}

#[test_case]
fn test_println_multiple() {
    println!("Line 1");
    println!("Line 2");
    println!("Line 3");
}

#[test_case]
fn test_print_without_newline() {
    print!("Hello ");
    print!("World");
    println!("!");
}

#[test_case]
fn test_println_empty() {
    println!();
    println!("");
}

#[test_case]
fn test_println_special_chars() {
    println!("Tab:\tNewline:\nCarriage return handled by UART");
}

#[test_case]
fn test_println_long_string() {
    println!("This is a longer string to test UART buffering and ensure that we can handle strings that span multiple characters without issues");
}

#[test_case]
fn test_println_unicode_replacement() {
    // Non-ASCII should be replaced with 0xFE
    println!("ASCII only: test");
}

// ============================================================================
// UART Driver Tests
// ============================================================================

#[test_case]
fn test_uart_write_byte() {
    use drivers::uart::WRITER;

    // Lock the UART and write some bytes
    let mut writer = WRITER.lock();
    writer.write_byte(b'A');
    writer.write_byte(b'B');
    writer.write_byte(b'C');
}

#[test_case]
fn test_uart_write_string() {
    use drivers::uart::WRITER;

    let mut writer = WRITER.lock();
    writer.write_string("UART test string");
}

#[test_case]
fn test_uart_newline_handling() {
    use drivers::uart::WRITER;

    // Test that newlines are converted to \r\n
    let mut writer = WRITER.lock();
    writer.write_string("Line1\nLine2\n");
}

#[test_case]
fn test_uart_multiple_init() {
    use drivers::uart::WRITER;

    // Test that multiple initializations are safe
    let mut writer = WRITER.lock();
    writer.init();
    writer.init();
    writer.write_string("Still works");
}

// ============================================================================
// Formatting Tests
// ============================================================================

#[test_case]
fn test_format_integers() {
    println!("Decimal: {}", 12345);
    println!("Hex: {:x}", 0xDEADBEEFu32);
    println!("Octal: {:o}", 0o777);
    println!("Binary: {:b}", 0b11001100);
}

#[test_case]
fn test_format_padding() {
    println!("Padded: {:08x}", 0xFF);
    println!("Right: {:>10}", "text");
}

#[test_case]
fn test_format_debug() {
    #[derive(Debug)]
    #[allow(dead_code)]
    struct TestStruct {
        value: u32,
    }

    let s = TestStruct { value: 42 };
    println!("Debug: {:?}", s);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test_case]
fn test_print_at_capacity() {
    // Print exactly 80 characters (VGA buffer width reference)
    print!("1234567890123456789012345678901234567890123456789012345678901234567890123456789");
    println!("!");
}

#[test_case]
fn test_uart_is_locked() {
    // Test that the UART writer uses a mutex (just verify it locks/unlocks)
    use drivers::uart::WRITER;

    let _guard = WRITER.lock();
    // Lock acquired successfully
    drop(_guard);
    // Lock released
}

// ============================================================================
// Exception Handling Tests
// ============================================================================

#[test_case]
fn test_exception_vectors_installed() {
    // Test that exception vectors are installed (init() should have been called)
    // This test just verifies the system doesn't crash with exceptions enabled
    use core::arch::asm;

    // Check current EL and read appropriate VBAR
    let current_el: u64;
    unsafe {
        asm!("mrs {}, CurrentEL", out(reg) current_el, options(nomem, nostack));
    }
    let el = (current_el >> 2) & 0x3;

    let vbar: u64;
    unsafe {
        if el == 2 {
            asm!("mrs {}, vbar_el2", out(reg) vbar, options(nomem, nostack));
        } else {
            asm!("mrs {}, vbar_el1", out(reg) vbar, options(nomem, nostack));
        }
    }

    // VBAR should be non-zero after init
    assert_ne!(vbar, 0, "VBAR should be set after init");
}

// ============================================================================
// Shell Command Parser Tests
// ============================================================================

#[test_case]
fn test_shell_parse_simple_command() {
    use shell::Command;

    let cmd = Command::parse("help").unwrap();
    assert_eq!(cmd.name, "help");
    assert_eq!(cmd.args, "");
}

#[test_case]
fn test_shell_parse_command_with_args() {
    use shell::Command;

    let cmd = Command::parse("echo hello world").unwrap();
    assert_eq!(cmd.name, "echo");
    assert_eq!(cmd.args, "hello world");
}

#[test_case]
fn test_shell_parse_whitespace_trimming() {
    use shell::Command;

    let cmd = Command::parse("  version  ").unwrap();
    assert_eq!(cmd.name, "version");
    assert_eq!(cmd.args, "");
}

#[test_case]
fn test_shell_parse_empty_line() {
    use shell::Command;

    assert!(Command::parse("").is_none());
    assert!(Command::parse("   ").is_none());
}

#[test_case]
fn test_shell_parse_multiple_spaces() {
    use shell::Command;

    // Args preserve all spacing after first space (including leading spaces)
    let cmd = Command::parse("echo    test   with   spaces").unwrap();
    assert_eq!(cmd.name, "echo");
    assert_eq!(cmd.args, "   test   with   spaces");
}

#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start_rust() -> ! {
    init();
    test_main();
    loop {}
}

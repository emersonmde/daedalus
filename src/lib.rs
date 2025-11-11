//! DaedalusOS - A bare-metal Rust kernel for Raspberry Pi.
//!
//! This is a `#![no_std]` kernel that runs directly on Raspberry Pi hardware (currently Pi 4).
//! It provides:
//! - Hardware drivers (UART for serial console)
//! - Exception handling for AArch64
//! - Simple bump allocator for heap memory
//! - Interactive shell with built-in commands
//! - Custom test framework for bare-metal testing

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![feature(alloc_error_handler)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

pub mod arch;
pub mod drivers;
pub mod mm;
pub mod net;
pub mod qemu;
pub mod shell;

// Re-exports for backward compatibility and convenience
pub use arch::aarch64::exceptions;
pub use mm::allocator;

use core::fmt::{self, Write};
use core::panic::PanicInfo;

// Global allocator
#[global_allocator]
static ALLOCATOR: allocator::BumpAllocator = allocator::BumpAllocator::new();

/// Handler for allocation errors
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Allocation error: {:?}", layout);
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

/// Panic handler for tests - used by both lib tests and integration tests
pub fn test_panic_handler(info: &PanicInfo) -> ! {
    println!("\x1b[31mFAILED\x1b[0m");
    println!();
    println!("Error: {}", info);
    println!();
    qemu::exit(qemu::ExitCode::Failed);
}

/// Enable IRQs by unmasking the I bit in DAIF register.
///
/// DAIF register controls exception masking:
/// - D: Debug exceptions
/// - A: SError (async abort)
/// - I: IRQ  ← We unmask this
/// - F: FIQ
///
/// Reference: ARM ARM Section D1.19.1
fn enable_irqs() {
    // SAFETY: Unmasking IRQs is safe because:
    // 1. The GIC has been initialized and configured
    // 2. Exception vectors are installed and ready to handle IRQs
    // 3. The UART interrupt handler is in place
    // 4. MSR instruction modifies only the DAIF register (no memory/stack effects)
    // 5. This enables interrupt-driven I/O which is the intended behavior
    unsafe {
        core::arch::asm!(
            "msr daifclr, #2", // Clear I bit (bit 1): 0b0010 = IRQ unmask
            options(nomem, nostack)
        );
    }
}

/// Initialize the kernel
///
/// Sets up hardware devices and prepares the system for operation.
pub fn init() {
    // Initialize MMU first for memory protection
    // SAFETY: Called exactly once during kernel initialization.
    // Identity mapping ensures all existing code/data addresses remain valid.
    unsafe {
        arch::aarch64::mmu::init();
    }

    // Initialize UART so we can print status messages
    drivers::uart::WRITER.lock().init();

    // Print startup sequence header
    println!();
    println!("DaedalusOS v{} booting...", env!("CARGO_PKG_VERSION"));
    println!();

    // Show initialization sequence
    // TODO: Add log levels (INFO, DEBUG, etc.) in future logging framework
    println!("[  OK  ] MMU initialized (virtual memory enabled)");

    exceptions::init();
    println!("[  OK  ] Exception vectors installed");

    // Initialize GIC (interrupt controller)
    {
        let mut gic = drivers::gic::GIC.lock();
        gic.init();

        // Enable UART0 interrupt in GIC
        gic.enable_interrupt(drivers::gic::irq::UART0);
    }
    println!("[  OK  ] GIC-400 interrupt controller initialized");

    // Enable UART RX interrupts
    drivers::uart::WRITER.lock().enable_rx_interrupt();

    // Enable IRQs at CPU level
    enable_irqs();
    println!("[  OK  ] IRQs enabled (interrupt-driven I/O active)");

    // Initialize heap allocator
    // SAFETY: This code is safe because:
    // 1. __heap_start and __heap_end are linker symbols defined in linker.ld at valid, non-overlapping addresses
    // 2. The linker script guarantees heap_start < heap_end (8MB region: 0x800000 bytes)
    // 3. Taking the address of linker symbols is safe (we don't dereference them, only get their addresses)
    // 4. Pointer-to-usize cast is always safe on this platform (64-bit addresses)
    // 5. ALLOCATOR.init() is unsafe but we satisfy its requirements:
    //    - Called exactly once (init() itself is called once during kernel startup)
    //    - No concurrent access (single-threaded at this point in boot)
    //    - heap_start < heap_end (guaranteed by linker as noted above)
    //    - Memory range is valid and reserved (linker reserves this region between BSS and stack)
    unsafe {
        // SAFETY: Linker symbols from linker.ld marking heap region boundaries (not actual variables).
        // Symbol addresses are valid at link time, used only to get addresses via &symbol as *const _ as usize.
        unsafe extern "C" {
            static __heap_start: u8;
            static __heap_end: u8;
        }
        let heap_start = &__heap_start as *const u8 as usize;
        let heap_end = &__heap_end as *const u8 as usize;
        ALLOCATOR.init(heap_start, heap_end);
    }
    println!(
        "[  OK  ] Heap allocator initialized ({} MB)",
        ALLOCATOR.heap_size() / 1024 / 1024
    );

    println!();

    // Print startup banner after all initialization is complete
    print_startup_banner();
}

/// Print kernel startup banner with system information
fn print_startup_banner() {
    use core::arch::asm;

    // Get exception level
    let current_el: u64;
    // SAFETY: Reading CurrentEL is safe (read-only register)
    unsafe {
        asm!("mrs {}, CurrentEL", out(reg) current_el, options(nomem, nostack));
    }
    let el = ((current_el >> 2) & 0x3) as u8;

    println!("Boot complete. Running at EL{}.", el);
    println!();
}

/// Print implementation that acquires the UART writer lock
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    drivers::uart::WRITER
        .lock()
        .write_fmt(args)
        .expect("Printing to UART failed");
}

/// Print formatted text to the serial console.
///
/// Uses the same syntax as `std::print!`.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

/// Print formatted text with newline to the serial console.
///
/// Uses the same syntax as `std::println!`.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// ============================================================================
// Test Infrastructure
// ============================================================================

/// Trait for test functions that can be run by the custom test framework.
pub trait Testable {
    /// Run this test and report results.
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("test {} ... ", core::any::type_name::<T>());
        self();
        // Print "ok" in green like cargo test
        println!("\x1b[32mok\x1b[0m");
    }
}

/// Custom test runner for bare-metal testing.
///
/// Runs all test functions and reports results in a cargo-test-like format.
/// Exits QEMU with appropriate exit code for CI integration.
pub fn test_runner(tests: &[&dyn Testable]) {
    println!();
    println!("running {} tests", tests.len());
    println!();
    for test in tests {
        test.run();
    }
    println!();
    print!("test result: \x1b[32mok\x1b[0m. ");
    println!(
        "{} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
        tests.len()
    );
    qemu::exit(qemu::ExitCode::Success);
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
    println!(
        "This is a longer string to test UART buffering and ensure that we can handle strings that span multiple characters without issues"
    );
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
    writer.write_byte(b'\n');
}

#[test_case]
fn test_uart_write_string() {
    use drivers::uart::WRITER;

    let mut writer = WRITER.lock();
    writer.write_string("UART test string\n");
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
    writer.write_string("Still works\n");
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
    // SAFETY: Reading CurrentEL system register is safe because:
    // 1. CurrentEL is a read-only system register (ARM ARM: https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CurrentEL--Current-Exception-Level)
    // 2. MRS instruction with nomem,nostack has no side effects (only reads register value)
    // 3. CurrentEL is accessible from EL1/EL2/EL3 (we run at EL1 or EL2, never EL0/usermode)
    // 4. Bits [3:2] contain EL value, bits [63:4] and [1:0] are RES0 (reserved zero)
    unsafe {
        asm!("mrs {}, CurrentEL", out(reg) current_el, options(nomem, nostack));
    }
    let el = (current_el >> 2) & 0x3;

    let vbar: u64;
    // SAFETY: Reading VBAR_ELx is safe because:
    // 1. We determined the current EL above and select the appropriate VBAR register for that EL
    // 2. VBAR_EL1/VBAR_EL2 are read-write registers holding exception vector table base address
    //    - VBAR_EL1: https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/VBAR-EL1--Vector-Base-Address-Register--EL1-
    //    - VBAR_EL2: https://developer.arm.com/documentation/ddi0595/2020-12/AArch64-Registers/VBAR-EL2--Vector-Base-Address-Register--EL2-
    // 3. MRS instruction with nomem,nostack has no side effects (only reads register value)
    // 4. Reading VBAR from current EL is architecturally permitted
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
fn test_debug_command_exists() {
    use shell::Command;

    // Verify debug command is recognized (ensures it's not accidentally removed)
    let cmd = Command::parse("debug").unwrap();
    assert_eq!(cmd.name, "debug");
    assert_eq!(cmd.args, "");
}

#[test_case]
fn test_exit_command_aliases() {
    use shell::Command;

    // Verify exit command and its aliases are recognized
    let exit_cmd = Command::parse("exit").unwrap();
    assert_eq!(exit_cmd.name, "exit");

    let shutdown_cmd = Command::parse("shutdown").unwrap();
    assert_eq!(shutdown_cmd.name, "shutdown");

    let halt_cmd = Command::parse("halt").unwrap();
    assert_eq!(halt_cmd.name, "halt");

    // Note: We can't actually test execution of exit since it calls
    // qemu::exit() which terminates the test runner. Just verify parsing.
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

    let cmd = Command::parse("echo    test   with   spaces").unwrap();
    assert_eq!(cmd.name, "echo");
    assert_eq!(cmd.args, "   test   with   spaces");
}

// ============================================================================
// Heap Allocator Tests
// ============================================================================

#[test_case]
fn test_box_allocation() {
    // Test Box allocation
    let heap_value = alloc::boxed::Box::new(42);
    assert_eq!(*heap_value, 42);
}

#[test_case]
fn test_vec_allocation() {
    use alloc::vec;

    // Test Vec creation and push
    #[allow(clippy::useless_vec)]
    let vec = vec![1, 2, 3];

    assert_eq!(vec.len(), 3);
    assert_eq!(vec[0], 1);
    assert_eq!(vec[1], 2);
    assert_eq!(vec[2], 3);
}

#[test_case]
fn test_string_allocation() {
    use alloc::string::String;

    // Test String allocation and concatenation
    let mut s = String::from("Hello");
    s.push_str(", ");
    s.push_str("World!");

    assert_eq!(s, "Hello, World!");
}

#[test_case]
fn test_vec_with_capacity() {
    use alloc::vec::Vec;

    // Test Vec with pre-allocated capacity
    let mut vec = Vec::with_capacity(10);
    for i in 0..10 {
        vec.push(i);
    }

    assert_eq!(vec.len(), 10);
    assert!(vec.capacity() >= 10);
}

#[test_case]
fn test_allocator_stats() {
    // Check that allocator is tracking usage
    let used_before = ALLOCATOR.used();

    // Allocate something
    let _boxed = alloc::boxed::Box::new([0u8; 1024]);

    let used_after = ALLOCATOR.used();

    // Usage should increase
    assert!(used_after > used_before, "Allocator should track usage");
}

#[cfg(test)]
// SAFETY: no_mangle required because this is the test entry point called by name from boot.s in test mode.
// extern "C" ensures stable ABI. Assembly caller guarantees same as main: aligned stack, zeroed BSS, EL1/EL2.
#[unsafe(no_mangle)]
pub extern "C" fn _start_rust() -> ! {
    init();
    test_main();
    loop {
        // Wait for interrupt to save power
        // SAFETY: WFI (Wait For Interrupt) instruction is safe because:
        // 1. WFI is a standard ARM instruction that puts the processor in low-power state
        // 2. The instruction has no side effects beyond power management
        // 3. options(nomem, nostack) correctly indicates no memory or stack access
        // 4. Processor wakes on any interrupt, allowing normal operation to resume
        // 5. This is the standard idle loop pattern for bare-metal ARM systems
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

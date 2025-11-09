//! Interactive shell (REPL) for DaedalusOS.
//!
//! Provides a simple command-line interface with built-in commands for
//! system information, memory statistics, and testing.

use crate::drivers::timer::SystemTimer;
use crate::drivers::uart::WRITER;
use crate::{ALLOCATOR, print, println};
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// Maximum line input buffer size in bytes.
const LINE_BUFFER_SIZE: usize = 256;

/// Kernel version from Cargo.toml.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum number of commands to keep in history.
const MAX_HISTORY: usize = 100;

/// Command history storage.
static HISTORY: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A parsed shell command with name and arguments.
pub struct Command<'a> {
    /// Command name (first word).
    pub name: &'a str,
    /// Arguments (everything after first word).
    pub args: &'a str,
}

impl<'a> Command<'a> {
    /// Parse a line into command name and arguments.
    ///
    /// Returns `None` if the line is empty or contains only whitespace.
    pub fn parse(line: &'a str) -> Option<Self> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        let mut parts = line.splitn(2, ' ');
        let name = parts.next()?;
        let args = parts.next().unwrap_or("");

        Some(Command { name, args })
    }
}

/// Read a line from UART with echo and basic line editing.
///
/// Supports backspace, Ctrl-C, and Ctrl-U (clear line).
/// Returns the number of bytes read into the buffer.
fn read_line(buffer: &mut [u8]) -> usize {
    let mut pos = 0;

    loop {
        let ch = {
            let mut writer = WRITER.lock();
            writer.read_byte()
        };

        match ch {
            // Enter (CR or LF)
            b'\r' | b'\n' => {
                println!();
                return pos;
            }

            // Backspace or Delete
            b'\x7f' | b'\x08' => {
                if pos > 0 {
                    pos -= 1;
                    // Move cursor back, write space, move cursor back again
                    print!("\x08 \x08");
                }
            }

            // Ctrl-C
            b'\x03' => {
                println!("^C");
                return 0;
            }

            // Ctrl-U (clear line)
            b'\x15' => {
                while pos > 0 {
                    print!("\x08 \x08");
                    pos -= 1;
                }
            }

            // Printable ASCII
            0x20..=0x7e => {
                if pos < buffer.len() {
                    buffer[pos] = ch;
                    pos += 1;
                    // Echo the character
                    print!("{}", ch as char);
                }
            }

            // Ignore other control characters
            _ => {}
        }
    }
}

/// Execute a built-in shell command.
fn execute_command(cmd: Command) {
    match cmd.name {
        "help" => {
            println!("DaedalusOS Shell Commands:");
            println!("  help      - Show this help message");
            println!("  echo      - Print arguments to console");
            println!("  clear     - Clear the screen");
            println!("  version   - Show kernel version");
            println!("  meminfo   - Display memory and heap statistics");
            println!("  uptime    - Show system uptime");
            println!("  history   - Show command history");
            println!("  debug     - Show system debug information");
            println!("  exit      - Shutdown system (exit QEMU or halt CPU)");
            println!("  exception - Trigger a breakpoint exception (for testing)");
        }

        "echo" => {
            println!("{}", cmd.args);
        }

        "clear" => {
            // ANSI escape sequence to clear screen and move cursor to top
            print!("\x1b[2J\x1b[H");
        }

        "version" => {
            println!("DaedalusOS version {}", VERSION);
            println!("Target: Raspberry Pi 4 (AArch64)");
        }

        "meminfo" => {
            let total = ALLOCATOR.heap_size();
            let used = ALLOCATOR.used();
            let free = ALLOCATOR.free();

            println!("Heap Statistics:");
            println!("  Total: {} bytes ({} MB)", total, total / 1024 / 1024);
            println!("  Used:  {} bytes ({} KB)", used, used / 1024);
            println!("  Free:  {} bytes ({} MB)", free, free / 1024 / 1024);
            println!("  Usage: {:.2}%", (used as f32 / total as f32) * 100.0);
        }

        "uptime" => {
            let uptime_us = SystemTimer::timestamp_us();
            let seconds = uptime_us / 1_000_000;
            let minutes = seconds / 60;
            let hours = minutes / 60;
            let days = hours / 24;

            if days > 0 {
                println!(
                    "Uptime: {} days, {} hours, {} minutes, {} seconds",
                    days,
                    hours % 24,
                    minutes % 60,
                    seconds % 60
                );
            } else if hours > 0 {
                println!(
                    "Uptime: {} hours, {} minutes, {} seconds",
                    hours,
                    minutes % 60,
                    seconds % 60
                );
            } else if minutes > 0 {
                println!("Uptime: {} minutes, {} seconds", minutes, seconds % 60);
            } else {
                println!("Uptime: {} seconds", seconds);
            }

            // Also show the raw microsecond count
            println!("  ({} microseconds)", uptime_us);
        }

        "history" => {
            let history = HISTORY.lock();
            if history.is_empty() {
                println!("No command history yet.");
            } else {
                println!("Command History:");
                for (i, cmd) in history.iter().enumerate() {
                    println!("  {}: {}", i + 1, cmd);
                }
            }
        }

        "debug" => {
            use core::arch::asm;

            println!("System Debug Information:");
            println!();

            // Exception Level
            let current_el: u64;
            // SAFETY: Reading CurrentEL is safe (read-only register)
            unsafe {
                asm!("mrs {}, CurrentEL", out(reg) current_el, options(nomem, nostack));
            }
            let el = ((current_el >> 2) & 0x3) as u8;
            println!("Exception Level: EL{}", el);

            // DAIF register (interrupt masks)
            let daif: u64;
            // SAFETY: Reading DAIF is safe (read-only access to current state)
            unsafe {
                asm!("mrs {}, DAIF", out(reg) daif, options(nomem, nostack));
            }
            println!("DAIF Register: 0x{:X}", daif);
            println!(
                "  D (Debug masked):    {}",
                if daif & (1 << 9) != 0 { "YES" } else { "NO" }
            );
            println!(
                "  A (SError masked):   {}",
                if daif & (1 << 8) != 0 { "YES" } else { "NO" }
            );
            println!(
                "  I (IRQ masked):      {}",
                if daif & (1 << 7) != 0 { "YES" } else { "NO" }
            );
            println!(
                "  F (FIQ masked):      {}",
                if daif & (1 << 6) != 0 { "YES" } else { "NO" }
            );

            // Vector Base Address Register
            let vbar: u64;
            // SAFETY: Reading VBAR is safe (read-only access)
            unsafe {
                if el == 2 {
                    asm!("mrs {}, vbar_el2", out(reg) vbar, options(nomem, nostack));
                } else {
                    asm!("mrs {}, vbar_el1", out(reg) vbar, options(nomem, nostack));
                }
            }
            println!("Vector Table (VBAR): 0x{:016X}", vbar);

            println!();

            // Timer counter
            let counter = SystemTimer::read_counter();
            let uptime_sec = counter / 1_000_000;
            println!("System Timer: {} us ({} sec)", counter, uptime_sec);

            println!();

            // Heap statistics (quick summary)
            let total = ALLOCATOR.heap_size();
            let used = ALLOCATOR.used();
            let free = ALLOCATOR.free();
            println!(
                "Heap: {} KB used, {} MB free / {} MB total ({:.1}%)",
                used / 1024,
                free / 1024 / 1024,
                total / 1024 / 1024,
                (used as f32 / total as f32) * 100.0
            );

            println!();

            // GIC status (just check if it's initialized by trying to read a register)
            println!("GIC-400 Interrupt Controller:");
            let gic = crate::drivers::gic::GIC.lock();
            // Note: We can't easily read "enabled interrupts" without adding accessors,
            // but we can show that it's initialized since we got the lock
            drop(gic);
            println!("  Status: Initialized");
            println!("  UART0 interrupt: ID {}", crate::drivers::gic::irq::UART0);

            println!();
            println!("Use 'meminfo' for detailed heap statistics");
        }

        "exit" | "shutdown" | "halt" => {
            use core::arch::asm;

            println!("Shutting down...");
            println!();

            // In QEMU: This will exit the emulator via semihosting
            // On real hardware: This will be ignored (semihosting not available)
            crate::qemu::exit(crate::qemu::ExitCode::Success);

            // Fallback for real hardware: Halt the CPU
            // Note: The code below is unreachable in QEMU (qemu::exit never returns),
            // but is executed on real hardware where semihosting is unavailable.
            #[allow(unreachable_code)]
            {
                println!("System halted. Safe to power off.");

                // Proper AArch64 halt sequence:
                // 1. Disable interrupts
                // 2. Memory barriers
                // 3. WFI loop
                //
                // Reference: ARM Architecture Reference Manual
                // Common pattern: MSR DAIFSET, #15; DSB SY; ISB SY; loop { WFI }
                //
                // SAFETY: This halt sequence is safe because:
                // 1. We're intentionally shutting down - no more work should be done
                // 2. Disabling interrupts prevents handlers from modifying state
                // 3. Memory barriers ensure all pending transactions complete
                // 4. WFI puts CPU in low-power mode (standard ARM halt pattern)
                unsafe {
                    asm!(
                        "msr daifset, #15", // Disable D, A, I, F interrupts
                        "dsb sy",           // Data Synchronization Barrier (full system)
                        "isb",              // Instruction Synchronization Barrier
                        "2:",               // Loop label
                        "wfi",              // Wait For Interrupt
                        "b 2b",             // Branch back to WFI (loop forever)
                        options(noreturn)
                    );
                }
            }
        }

        "exception" => {
            println!("Triggering breakpoint exception...");
            // Trigger a BRK instruction which will cause a synchronous exception
            // SAFETY: BRK instruction is safe because:
            // 1. BRK #0 is a valid AArch64 instruction that triggers a synchronous software breakpoint exception
            // 2. Exception handlers are installed by init() before the shell runs (pre-condition: init() called)
            // 3. The exception handler will catch this and display exception info, then panic
            // 4. options(nostack) correctly indicates this instruction doesn't access the stack
            // 5. This is the intended behavior for the "exception" test command
            unsafe {
                core::arch::asm!("brk #0", options(nostack));
            }
        }

        _ => {
            println!("Unknown command: {}", cmd.name);
            println!("Type 'help' for available commands.");
        }
    }
}

/// Run the interactive shell REPL (Read-Eval-Print Loop).
///
/// This function never returns - it loops forever reading and executing commands.
pub fn run() -> ! {
    let mut line_buffer = [0u8; LINE_BUFFER_SIZE];

    println!();
    println!("Welcome to DaedalusOS!");
    println!("Type 'help' for available commands.");
    println!();

    loop {
        print!("daedalus> ");
        let len = read_line(&mut line_buffer);

        if len == 0 {
            continue;
        }

        // Convert buffer to str (safe because we only accept ASCII in read_line)
        if let Ok(line) = core::str::from_utf8(&line_buffer[..len])
            && let Some(cmd) = Command::parse(line)
        {
            // Add command to history (skip 'history' command itself to avoid clutter)
            if cmd.name != "history" {
                let mut history = HISTORY.lock();
                history.push(String::from(line));

                // Keep history size limited
                if history.len() > MAX_HISTORY {
                    history.remove(0);
                }
            }

            execute_command(cmd);
        }
    }
}

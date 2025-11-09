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

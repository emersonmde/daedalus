//! Interactive shell (REPL) for DaedalusOS.
//!
//! Provides a simple command-line interface with built-in commands for
//! system information, memory statistics, and testing.

use crate::drivers::genet::GENET;
use crate::drivers::gpio::{Function, Gpio, Pull};
use crate::drivers::timer::SystemTimer;
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
        let ch = loop {
            if let Some(byte) = crate::drivers::uart::read_rx_byte() {
                break byte;
            }
            core::hint::spin_loop();
        };

        match ch {
            b'\r' | b'\n' => {
                println!();
                return pos;
            }

            b'\x7f' | b'\x08' => {
                if pos > 0 {
                    pos -= 1;
                    print!("\x08 \x08");
                }
            }

            b'\x03' => {
                println!("^C");
                return 0;
            }

            b'\x15' => {
                while pos > 0 {
                    print!("\x08 \x08");
                    pos -= 1;
                }
            }

            0x20..=0x7e => {
                if pos < buffer.len() {
                    buffer[pos] = ch;
                    pos += 1;
                    print!("{}", ch as char);
                }
            }

            _ => {}
        }
    }
}

/// Execute a built-in shell command.
fn execute_command(cmd: Command) {
    match cmd.name {
        "help" => {
            println!("DaedalusOS Shell Commands:");
            println!();
            println!("System:");
            println!("  help           - Show this help message");
            println!("  version        - Show kernel version");
            println!("  exit           - Shutdown system (exit QEMU or halt CPU)");
            println!();
            println!("Information:");
            println!("  meminfo        - Display memory and heap statistics");
            println!("  uptime         - Show system uptime");
            println!("  debug          - Show system debug information");
            println!("  mmu            - Show MMU (virtual memory) status");
            println!("  history        - Show command history");
            println!();
            println!("GPIO:");
            println!("  gpio-mode <pin> <mode>   - Set pin function (input/output/alt0-5)");
            println!("  gpio-pull <pin> <mode>   - Set pull resistor (none/up/down)");
            println!("  gpio-set <pin> <value>   - Set output pin (1=high, 0=low)");
            println!("  gpio-get <pin>           - Read pin level");
            println!("  gpio-toggle <pin>        - Toggle output pin");
            println!();
            println!("Network:");
            println!("  eth-stats      - Show Ethernet packet statistics");
            println!("  netstats       - Show network stack debug statistics");
            println!("  arp-probe      - Send ARP probe to test TX/RX (10.42.10.1)");
            println!("  fetch-kernel   - Download kernel from dev server (10.42.10.100:8000)");
            println!();
            println!("Utilities:");
            println!("  echo           - Print arguments to console");
            println!("  clear          - Clear the screen");
            println!();
            println!("Testing:");
            println!("  exception      - Trigger a breakpoint exception");
            println!("  kexec <addr>   - Jump to kernel at address (DANGEROUS!)");
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

            // GIC status
            // Note: We don't lock GIC here to avoid potential deadlock if interrupt fires
            // (IRQ handler needs GIC lock to acknowledge/EOI interrupts)
            println!("GIC-400 Interrupt Controller:");
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

        "mmu" => {
            use crate::arch::aarch64::mmu;

            println!("MMU (Memory Management Unit) Status:");
            println!();

            // Check if MMU is enabled
            let enabled = mmu::is_enabled();
            println!("  Status: {}", if enabled { "ENABLED" } else { "DISABLED" });

            if enabled {
                // Show MMU configuration registers
                let ttbr0 = mmu::get_ttbr0();
                let tcr = mmu::get_tcr();
                let mair = mmu::get_mair();

                println!();
                println!("  Translation Table Base (TTBR0_EL1): 0x{:016X}", ttbr0);

                println!();
                println!("  Translation Control (TCR_EL1): 0x{:016X}", tcr);
                let t0sz = tcr & 0x3F;
                let granule = match (tcr >> 14) & 0x3 {
                    0b00 => "4 KB",
                    0b01 => "64 KB",
                    0b10 => "16 KB",
                    _ => "Reserved",
                };
                let va_bits = 64 - t0sz;
                let va_size = 1u64 << va_bits;
                println!(
                    "    Virtual address size: {} bits ({} GB)",
                    va_bits,
                    va_size / (1024 * 1024 * 1024)
                );
                println!("    Page granule: {}", granule);

                println!();
                println!("  Memory Attributes (MAIR_EL1): 0x{:016X}", mair);
                let attr0 = mair & 0xFF;
                let attr1 = (mair >> 8) & 0xFF;
                println!("    Attr0 (Device): 0x{:02X} (Device-nGnRnE)", attr0);
                println!(
                    "    Attr1 (Normal):  0x{:02X} (Normal WB RW-Allocate)",
                    attr1
                );

                println!();
                println!("  Memory Mappings (Identity):");
                println!("    0x00000000-0x3FFFFFFF → Normal memory (kernel + DRAM)");
                println!("    0xFE000000-0xFF800000 → Device memory (MMIO)");
            } else {
                println!("  (Virtual memory is not active - using physical addresses)");
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

        "gpio-mode" => {
            let mut parts = cmd.args.split_whitespace();
            let pin_str = parts.next();
            let mode_str = parts.next();

            if let (Some(pin_str), Some(mode_str)) = (pin_str, mode_str) {
                if let Ok(pin) = pin_str.parse::<u32>() {
                    if pin >= 58 {
                        println!("Error: GPIO pin must be 0-57");
                        return;
                    }

                    let function = match mode_str.to_lowercase().as_str() {
                        "input" | "in" => Function::Input,
                        "output" | "out" => Function::Output,
                        "alt0" => Function::Alt0,
                        "alt1" => Function::Alt1,
                        "alt2" => Function::Alt2,
                        "alt3" => Function::Alt3,
                        "alt4" => Function::Alt4,
                        "alt5" => Function::Alt5,
                        _ => {
                            println!("Error: Invalid mode '{}'", mode_str);
                            println!(
                                "Valid modes: input, output, alt0, alt1, alt2, alt3, alt4, alt5"
                            );
                            return;
                        }
                    };

                    let gpio = Gpio::new();
                    gpio.set_function(pin, function);
                    println!("GPIO {} set to {:?}", pin, function);
                } else {
                    println!("Error: Invalid pin number '{}'", pin_str);
                }
            } else {
                println!("Usage: gpio-mode <pin> <mode>");
                println!("Example: gpio-mode 42 output");
            }
        }

        "gpio-pull" => {
            let mut parts = cmd.args.split_whitespace();
            let pin_str = parts.next();
            let mode_str = parts.next();

            if let (Some(pin_str), Some(mode_str)) = (pin_str, mode_str) {
                if let Ok(pin) = pin_str.parse::<u32>() {
                    if pin >= 58 {
                        println!("Error: GPIO pin must be 0-57");
                        return;
                    }

                    let pull = match mode_str.to_lowercase().as_str() {
                        "none" | "off" | "disable" => Pull::None,
                        "up" | "pullup" | "pull-up" => Pull::Up,
                        "down" | "pulldown" | "pull-down" => Pull::Down,
                        _ => {
                            println!("Error: Invalid pull mode '{}'", mode_str);
                            println!("Valid modes: none, up, down");
                            return;
                        }
                    };

                    let gpio = Gpio::new();
                    gpio.set_pull(pin, pull);
                    println!("GPIO {} pull resistor set to {:?}", pin, pull);
                } else {
                    println!("Error: Invalid pin number '{}'", pin_str);
                }
            } else {
                println!("Usage: gpio-pull <pin> <mode>");
                println!("Example: gpio-pull 17 up");
            }
        }

        "gpio-set" => {
            let mut parts = cmd.args.split_whitespace();
            let pin_str = parts.next();
            let value_str = parts.next();

            if let (Some(pin_str), Some(value_str)) = (pin_str, value_str) {
                if let Ok(pin) = pin_str.parse::<u32>() {
                    if pin >= 58 {
                        println!("Error: GPIO pin must be 0-57");
                        return;
                    }

                    let value = match value_str {
                        "1" | "high" | "on" | "true" => true,
                        "0" | "low" | "off" | "false" => false,
                        _ => {
                            println!("Error: Invalid value '{}'", value_str);
                            println!("Valid values: 1/high/on or 0/low/off");
                            return;
                        }
                    };

                    let gpio = Gpio::new();
                    gpio.write(pin, value);
                    println!("GPIO {} set to {}", pin, if value { "HIGH" } else { "LOW" });
                } else {
                    println!("Error: Invalid pin number '{}'", pin_str);
                }
            } else {
                println!("Usage: gpio-set <pin> <value>");
                println!("Example: gpio-set 42 1");
            }
        }

        "gpio-get" => {
            let pin_str = cmd.args.trim();

            if !pin_str.is_empty() {
                if let Ok(pin) = pin_str.parse::<u32>() {
                    if pin >= 58 {
                        println!("Error: GPIO pin must be 0-57");
                        return;
                    }

                    let gpio = Gpio::new();
                    let value = gpio.read(pin);
                    println!(
                        "GPIO {} = {} ({})",
                        pin,
                        if value { 1 } else { 0 },
                        if value { "HIGH" } else { "LOW" }
                    );
                } else {
                    println!("Error: Invalid pin number '{}'", pin_str);
                }
            } else {
                println!("Usage: gpio-get <pin>");
                println!("Example: gpio-get 17");
            }
        }

        "gpio-toggle" => {
            let pin_str = cmd.args.trim();

            if !pin_str.is_empty() {
                if let Ok(pin) = pin_str.parse::<u32>() {
                    if pin >= 58 {
                        println!("Error: GPIO pin must be 0-57");
                        return;
                    }

                    let gpio = Gpio::new();
                    gpio.toggle(pin);
                    let new_value = gpio.read(pin);
                    println!(
                        "GPIO {} toggled to {} ({})",
                        pin,
                        if new_value { 1 } else { 0 },
                        if new_value { "HIGH" } else { "LOW" }
                    );
                } else {
                    println!("Error: Invalid pin number '{}'", pin_str);
                }
            } else {
                println!("Usage: gpio-toggle <pin>");
                println!("Example: gpio-toggle 42");
            }
        }

        "eth-stats" => {
            let genet = GENET.lock();

            if !genet.is_present() {
                println!("[ERROR] GENET hardware not detected!");
                return;
            }

            let stats = genet.read_stats();

            println!("Ethernet Statistics:");
            println!();
            println!("TX (Transmit):");
            println!("  Packets:   {}", stats.tx_packets);
            println!("  Bytes:     {}", stats.tx_bytes);
            println!("  Broadcast: {}", stats.tx_broadcast);
            println!("  Multicast: {}", stats.tx_multicast);
            println!();
            println!("RX (Receive):");
            println!("  Packets:   {}", stats.rx_packets);
            println!("  Bytes:     {}", stats.rx_bytes);
            println!("  Unicast:   {}", stats.rx_unicast);
            println!("  Broadcast: {}", stats.rx_broadcast);
            println!("  Multicast: {}", stats.rx_multicast);
            println!();
            println!("RX Errors:");
            println!("  FCS errors:       {}", stats.rx_fcs_errors);
            println!("  Alignment errors: {}", stats.rx_align_errors);
        }

        "netstats" => {
            use crate::net::router;
            router::print_debug_stats();
        }

        "arp-probe" => {
            use crate::net::run_arp_probe_diagnostic;
            run_arp_probe_diagnostic();
        }

        "fetch-kernel" => {
            use crate::arch::aarch64::kexec;
            use crate::net::http;

            // Hardcoded dev server IP (10.42.10.100:8000)
            const DEV_SERVER: [u8; 4] = [10, 42, 10, 100];
            const DEV_PORT: u16 = 8000;

            println!("Fetching kernel from 10.42.10.100:8000...");

            match http::get_binary(DEV_SERVER, DEV_PORT, "/kernel") {
                Ok(kernel_data) => {
                    println!("Downloaded {} bytes", kernel_data.len());

                    // Stage kernel to network staging area
                    unsafe {
                        match kexec::stage_kernel(&kernel_data) {
                            Ok(staging_addr) => {
                                println!("Kernel staged at 0x{:08x}", staging_addr);
                                println!("Run 'kexec 0x{:08x}' to boot it", staging_addr);
                            }
                            Err(e) => {
                                println!("Failed to stage kernel: {:?}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Fetch failed: {:?}", e);
                    println!("Make sure:");
                    println!(
                        "  1. Dev server is running: cd daedalus-dev-server && cargo run -- ../target/.../kernel8.img"
                    );
                    println!(
                        "  2. Network is configured: Pi at 10.42.10.42, Server at 10.42.10.100"
                    );
                    println!("  3. Ethernet cable is connected");
                }
            }
        }

        "kexec" => {
            use crate::arch::aarch64::kexec;
            use crate::boot_mode::BootMode;

            let addr_str = cmd.args.trim();
            if addr_str.is_empty() {
                println!("Usage: kexec <address>");
                println!("Example: kexec 0x01000000");
                println!();
                println!("WARNING: This command will jump to arbitrary memory and never return!");
                println!("         Only use for testing kexec functionality.");
                return;
            }

            // Parse address (support hex with 0x prefix or decimal)
            let addr = if let Some(hex) = addr_str.strip_prefix("0x") {
                usize::from_str_radix(hex, 16)
            } else if let Some(hex) = addr_str.strip_prefix("0X") {
                usize::from_str_radix(hex, 16)
            } else {
                addr_str.parse::<usize>()
            };

            match addr {
                Ok(addr) => {
                    println!("Performing kexec to address 0x{:08X}", addr);
                    println!("Current boot mode: {}", BootMode::detect());
                    println!();
                    println!("WARNING: System will jump to new kernel in 3 seconds...");

                    // Give user time to see the message
                    for i in (1..=3).rev() {
                        SystemTimer::delay_ms(1000);
                        println!("  {}...", i);
                    }

                    println!("Jumping to new kernel!");
                    println!();

                    // Get current DTB pointer from device tree module
                    // We stored this during boot, so it should be available
                    let dtb_ptr = crate::dt::get_dtb_pointer();

                    // SAFETY: This is EXTREMELY unsafe! We're jumping to arbitrary memory.
                    // The user has been warned, and this is a test command.
                    // In a real system, we would validate the kernel before jumping.
                    unsafe {
                        kexec::kexec(
                            addr,
                            4 * 1024 * 1024, // Assume 4MB kernel (validation will check)
                            dtb_ptr,
                        );
                    }
                    // Never returns
                }
                Err(_) => {
                    println!("Error: Invalid address '{}'", addr_str);
                    println!("Address must be a hex number (0x...) or decimal number");
                }
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

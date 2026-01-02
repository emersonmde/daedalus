//! PL011 UART driver for serial console I/O.
//!
//! Provides a simple polling-based UART driver for the Raspberry Pi's PL011 UART0.
//! Supports both transmit and receive operations with proper FIFO handling.

use crate::sync::Mutex;
use core::fmt;
use core::sync::atomic::{AtomicUsize, Ordering};
use lazy_static::lazy_static;
use volatile::Volatile;

/// PL011 UART base address (BCM2711 Low Peripheral mode).
///
/// Public for device tree verification.
pub const UART_BASE: usize = 0xFE20_1000;

/// PL011 UART register bit definitions.
///
/// Reference: [ARM PL011 TRM](https://developer.arm.com/documentation/ddi0183/latest/)
mod pl011_flags {
    // Flag Register (FR) bits - Section 3.3.6
    pub const FR_TXFF: u32 = 1 << 5; // Transmit FIFO full
    pub const FR_RXFE: u32 = 1 << 4; // Receive FIFO empty

    // Line Control Register (LCRH) bits - Section 3.3.7
    pub const LCRH_FEN: u32 = 1 << 4; // FIFO enable
    pub const LCRH_WLEN_8BIT: u32 = 0b11 << 5; // 8-bit word length

    // Control Register (CR) bits - Section 3.3.8
    pub const CR_UARTEN: u32 = 1 << 0; // UART enable
    pub const CR_TXE: u32 = 1 << 8; // Transmit enable
    pub const CR_RXE: u32 = 1 << 9; // Receive enable

    // Data Register (DR) bits - Section 3.3.1
    pub const DR_DATA_MASK: u32 = 0xFF; // Data bits [7:0]

    // Interrupt Mask Set/Clear Register (IMSC) - Section 3.3.11
    pub const IMSC_RXIM: u32 = 1 << 4; // Receive interrupt mask
    pub const IMSC_RTIM: u32 = 1 << 6; // Receive timeout interrupt mask

    // Masked Interrupt Status Register (MIS) - Section 3.3.12
    #[allow(dead_code)]
    pub const MIS_RXMIS: u32 = 1 << 4; // Receive masked interrupt status
    #[allow(dead_code)]
    pub const MIS_RTMIS: u32 = 1 << 6; // Receive timeout masked interrupt status

    // Interrupt Clear Register (ICR) - Section 3.3.13
    pub const ICR_RXIC: u32 = 1 << 4; // Receive interrupt clear
    pub const ICR_RTIC: u32 = 1 << 6; // Receive timeout interrupt clear
    pub const ICR_ALL: u32 = 0x7FF; // Clear all interrupts

    // Interrupt FIFO Level Select Register (IFLS) - Section 3.3.10
    // Source: ARM PrimeCell UART PL011 TRM
    // RXIFLSEL (bits 5:3) - Receive interrupt FIFO level
    pub const IFLS_RXIFLSEL_1_8: u32 = 0b000 << 3; // RX FIFO ≥ 1/8 full (2 bytes)
    #[allow(dead_code)]
    pub const IFLS_RXIFLSEL_1_4: u32 = 0b001 << 3; // RX FIFO ≥ 1/4 full (4 bytes)
    #[allow(dead_code)]
    pub const IFLS_RXIFLSEL_1_2: u32 = 0b010 << 3; // RX FIFO ≥ 1/2 full (8 bytes)
    #[allow(dead_code)]
    pub const IFLS_RXIFLSEL_3_4: u32 = 0b011 << 3; // RX FIFO ≥ 3/4 full (12 bytes)
    #[allow(dead_code)]
    pub const IFLS_RXIFLSEL_7_8: u32 = 0b100 << 3; // RX FIFO ≥ 7/8 full (14 bytes)

    // TXIFLSEL (bits 2:0) - Transmit interrupt FIFO level
    pub const IFLS_TXIFLSEL_1_8: u32 = 0b000; // TX FIFO ≤ 1/8 full (2 bytes)

    // Combined RX-only configuration (recommend 1/8 for low latency)
    pub const IFLS_RX_1_8: u32 = IFLS_RXIFLSEL_1_8 | IFLS_TXIFLSEL_1_8;
}

/// Lock-free SPSC ring buffer for UART RX data
///
/// Single producer: Interrupt handler (writes bytes)
/// Single consumer: Shell read_line() (reads bytes)
struct RxRingBuffer {
    buffer: [u8; Self::CAPACITY],
    head: AtomicUsize, // Write index (producer only)
    tail: AtomicUsize, // Read index (consumer only)
}

impl RxRingBuffer {
    /// Buffer capacity - MUST be power of 2 for efficient modulo
    pub const CAPACITY: usize = 512;

    /// Bit mask for wrapping indices
    const MASK: usize = Self::CAPACITY - 1;

    pub const fn new() -> Self {
        Self {
            buffer: [0u8; Self::CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Enqueue a byte (called from interrupt handler)
    /// Returns false if buffer full
    pub fn enqueue(&self, byte: u8) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        let next_head = (head + 1) & Self::MASK;

        if next_head == tail {
            return false;
        }

        // SAFETY: head < CAPACITY due to masking, and this is the only writer
        unsafe {
            let buffer_ptr = self.buffer.as_ptr() as *mut u8;
            buffer_ptr.add(head).write(byte);
        }

        self.head.store(next_head, Ordering::Release);
        true
    }

    /// Dequeue a byte (called from shell)
    /// Returns Some(byte) if available, None if empty
    pub fn dequeue(&self) -> Option<u8> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if head == tail {
            return None;
        }

        // SAFETY: tail < CAPACITY due to masking, and this is the only reader
        let byte = unsafe {
            let buffer_ptr = self.buffer.as_ptr();
            buffer_ptr.add(tail).read()
        };

        let next_tail = (tail + 1) & Self::MASK;

        self.tail.store(next_tail, Ordering::Release);

        Some(byte)
    }

    /// Check if buffer is empty (racy but safe for diagnostics)
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        head == tail
    }
}

// SAFETY: Safe to share across threads (interrupt and main):
// - Single producer modifies head, single consumer modifies tail (no data races)
// - Acquire/Release ordering prevents reordering across synchronization points
// - Buffer indices are always in bounds due to power-of-2 masking
unsafe impl Sync for RxRingBuffer {}

/// Global RX ring buffer (interrupt handler → shell)
static RX_BUFFER: RxRingBuffer = RxRingBuffer::new();

lazy_static! {
    pub static ref WRITER: Mutex<UartWriter> = Mutex::new(UartWriter::new());
}

/// PL011 UART register offsets
#[repr(C)]
struct Pl011Registers {
    dr: Volatile<u32>, // 0x00 - Data Register
    _rsv0: [u32; 5],
    fr: Volatile<u32>, // 0x18 - Flag Register
    _rsv1: [u32; 2],
    ibrd: Volatile<u32>, // 0x24 - Integer Baud Rate Divisor
    fbrd: Volatile<u32>, // 0x28 - Fractional Baud Rate Divisor
    lcrh: Volatile<u32>, // 0x2C - Line Control Register
    cr: Volatile<u32>,   // 0x30 - Control Register
    ifls: Volatile<u32>, // 0x34 - Interrupt FIFO Level Select
    imsc: Volatile<u32>, // 0x38 - Interrupt Mask Set/Clear
    _rsv3: [u32; 1],
    mis: Volatile<u32>, // 0x40 - Masked Interrupt Status
    icr: Volatile<u32>, // 0x44 - Interrupt Clear Register
}

/// UART writer for serial console output
pub struct UartWriter {
    registers: &'static mut Pl011Registers,
    initialized: bool,
}

impl Default for UartWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl UartWriter {
    /// Create a new UART writer instance
    pub const fn new() -> Self {
        UartWriter {
            // SAFETY: Creating a reference to MMIO registers is safe because:
            // 1. UART_BASE (0xFE201000) is the documented PL011 UART0 base address for RPi4:
            //    - BCM2711 peripherals start at 0xFE000000 in Low Peripheral Mode (default)
            //    - UART0 offset is 0x00201000 from peripheral base
            //    - Reference: https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf
            // 2. This address is reserved by the hardware and will never be used for other purposes
            // 3. The memory-mapped registers are always present and accessible
            // 4. Pl011Registers struct matches the PL011 register layout exactly:
            //    - Reference: ARM PL011 TRM https://developer.arm.com/documentation/ddi0183/latest/
            //    - See Section 3.2 "Summary of registers" for register offsets
            // 5. UartWriter is wrapped in a Mutex (WRITER static), ensuring exclusive access
            // 6. This is a const fn, so the reference is created once at compile-time/static init
            registers: unsafe { &mut *(UART_BASE as *mut Pl011Registers) },
            initialized: false,
        }
    }

    /// Initialize the UART hardware
    ///
    /// Configures for 115200 baud, 8N1, FIFO enabled
    /// Based on 48 MHz UART clock (IBRD=26, FBRD=3) for Pi 4 hardware
    pub fn init(&mut self) {
        // Small delay helper for hardware stabilization
        fn small_delay() {
            for _ in 0..150 {
                unsafe { core::arch::asm!("nop", options(nomem, nostack)) };
            }
        }

        // Configure GPIO 14 (TXD0) and GPIO 15 (RXD0) for UART0 (Alt0 function)
        // Required on real hardware - firmware doesn't always set this up
        // SAFETY: Writing to GPIO registers is safe because:
        // 1. GPFSEL1 address is correct for BCM2711
        // 2. We're only modifying GPIO 14/15 function bits, preserving others
        // 3. Alt0 is the correct function for UART0 on these pins
        unsafe {
            use core::ptr::{read_volatile, write_volatile};
            const GPFSEL1: usize = 0xFE200004; // GPIO Function Select 1

            let mut fsel = read_volatile(GPFSEL1 as *const u32);
            fsel &= !((0b111 << 12) | (0b111 << 15)); // Clear GPIO 14 and 15 function bits
            fsel |= (0b100 << 12) | (0b100 << 15); // Set both to Alt0
            write_volatile(GPFSEL1 as *mut u32, fsel);
        }

        // Disable UART during configuration
        self.registers.cr.write(0);
        small_delay(); // Hardware needs time to disable

        // Mask all interrupts
        self.registers.imsc.write(0);

        // Clear all pending interrupts
        self.registers.icr.write(pl011_flags::ICR_ALL);
        small_delay(); // Let FIFOs flush

        // Set baud rate divisors for 115200 @ 48 MHz (Pi 4 hardware clock)
        // QEMU uses 54 MHz, but real hardware uses 48 MHz UART clock
        // Divisor = 48,000,000 / (16 * 115200) = 26.0416...
        // Integer part = 26, Fractional part = 0.0416 * 64 + 0.5 = 3
        self.registers.ibrd.write(26);
        self.registers.fbrd.write(3);

        // Configure line control: 8 bits, FIFO enabled, no parity
        self.registers
            .lcrh
            .write(pl011_flags::LCRH_FEN | pl011_flags::LCRH_WLEN_8BIT);

        small_delay(); // Stabilize before enabling

        // Enable UART, TX, and RX
        self.registers
            .cr
            .write(pl011_flags::CR_UARTEN | pl011_flags::CR_TXE | pl011_flags::CR_RXE);

        small_delay(); // Let UART stabilize after enabling

        self.initialized = true;
    }

    /// Write a single byte to the UART
    pub fn write_byte(&mut self, byte: u8) {
        if !self.initialized {
            self.init();
        }

        // Wait until transmit FIFO is not full
        while (self.registers.fr.read() & pl011_flags::FR_TXFF) != 0 {}

        // Write the byte
        self.registers.dr.write(byte as u32);
    }

    /// Write a string to the UART
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            // Convert newline to carriage return + newline
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
    }

    /// Read a single byte from the UART (blocking)
    ///
    /// Polls the receive FIFO until a character is available.
    /// Returns the received byte.
    pub fn read_byte(&mut self) -> u8 {
        if !self.initialized {
            self.init();
        }

        // Wait until receive FIFO is not empty
        while (self.registers.fr.read() & pl011_flags::FR_RXFE) != 0 {}

        // Read and return the byte (only lower 8 bits are data)
        (self.registers.dr.read() & pl011_flags::DR_DATA_MASK) as u8
    }

    /// Enable receive interrupts
    ///
    /// Enables both RX interrupt (fires when data is received) and
    /// receive timeout interrupt (fires when FIFO has data but no new
    /// characters arrive for a timeout period).
    pub fn enable_rx_interrupt(&mut self) {
        if !self.initialized {
            self.init();
        }

        // Enable RX and RX timeout interrupts
        let mut imsc = self.registers.imsc.read();
        imsc |= pl011_flags::IMSC_RXIM | pl011_flags::IMSC_RTIM;
        self.registers.imsc.write(imsc);
    }

    /// Disable receive interrupts
    pub fn disable_rx_interrupt(&mut self) {
        // Disable RX and RX timeout interrupts
        let mut imsc = self.registers.imsc.read();
        imsc &= !(pl011_flags::IMSC_RXIM | pl011_flags::IMSC_RTIM);
        self.registers.imsc.write(imsc);
    }

    /// Configure RX FIFO interrupt threshold to 1/8 full (2 bytes)
    pub fn set_rx_fifo_threshold(&mut self) {
        if !self.initialized {
            self.init();
        }

        self.registers.ifls.write(pl011_flags::IFLS_RX_1_8);
    }

    /// Clear pending RX interrupts
    pub fn clear_rx_interrupts(&mut self) {
        self.registers
            .icr
            .write(pl011_flags::ICR_RXIC | pl011_flags::ICR_RTIC);
    }
}

impl fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Handle UART receive interrupt
///
/// Drains FIFO and enqueues bytes to ring buffer.
pub fn handle_interrupt() {
    // Drain FIFO and clear interrupt while holding lock
    let bytes_dropped = {
        let mut writer = WRITER.lock();
        let mut bytes_dropped = 0;

        while (writer.registers.fr.read() & pl011_flags::FR_RXFE) == 0 {
            let byte = (writer.registers.dr.read() & pl011_flags::DR_DATA_MASK) as u8;

            if !RX_BUFFER.enqueue(byte) {
                bytes_dropped += 1;
            }
        }

        writer
            .registers
            .icr
            .write(pl011_flags::ICR_RXIC | pl011_flags::ICR_RTIC);

        bytes_dropped
    }; // Lock dropped here - safe to call println! now

    // Report buffer overflows (rare, rate-limited)
    // IMPORTANT: Must be outside the WRITER lock to avoid deadlock
    if bytes_dropped > 0 {
        static OVERFLOW_COUNT: core::sync::atomic::AtomicU32 =
            core::sync::atomic::AtomicU32::new(0);
        let count = OVERFLOW_COUNT.fetch_add(1, Ordering::Relaxed);

        if count < 5 {
            crate::println!(
                "[UART] RX buffer overflow: {} bytes dropped (event #{})",
                bytes_dropped,
                count + 1
            );
        }
    }
}

/// Read a byte from RX ring buffer (non-blocking)
pub fn read_rx_byte() -> Option<u8> {
    RX_BUFFER.dequeue()
}

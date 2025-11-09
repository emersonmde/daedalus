//! PL011 UART driver for serial console I/O.
//!
//! Provides a simple polling-based UART driver for the Raspberry Pi's PL011 UART0.
//! Supports both transmit and receive operations with proper FIFO handling.

use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;

/// PL011 UART base address (BCM2711 Low Peripheral mode).
const UART_BASE: usize = 0xFE20_1000;

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

    // Interrupt Clear Register (ICR) - Section 3.3.13
    pub const ICR_ALL: u32 = 0x7FF; // Clear all interrupts
}

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
    _rsv2: [u32; 1],
    imsc: Volatile<u32>, // 0x38 - Interrupt Mask Set/Clear
    _rsv3: [u32; 2],
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
    /// Based on 54 MHz UART clock (IBRD=29, FBRD=19)
    pub fn init(&mut self) {
        // Disable UART
        self.registers.cr.write(0);

        // Mask all interrupts
        self.registers.imsc.write(0);

        // Clear all pending interrupts
        self.registers.icr.write(pl011_flags::ICR_ALL);

        // Set baud rate divisors for 115200 @ 54 MHz
        // Divisor = 54,000,000 / (16 * 115200) = 29.296875
        // Integer part = 29, Fractional part = 0.296875 * 64 = 19
        self.registers.ibrd.write(29);
        self.registers.fbrd.write(19);

        // Configure line control: 8 bits, FIFO enabled, no parity
        self.registers
            .lcrh
            .write(pl011_flags::LCRH_FEN | pl011_flags::LCRH_WLEN_8BIT);

        // Enable UART, TX, and RX
        self.registers
            .cr
            .write(pl011_flags::CR_UARTEN | pl011_flags::CR_TXE | pl011_flags::CR_RXE);

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
}

impl fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

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

    // Interrupt Mask Set/Clear Register (IMSC) - Section 3.3.11
    pub const IMSC_RXIM: u32 = 1 << 4; // Receive interrupt mask
    pub const IMSC_RTIM: u32 = 1 << 6; // Receive timeout interrupt mask

    // Masked Interrupt Status Register (MIS) - Section 3.3.12
    #[allow(dead_code)]
    pub const MIS_RXMIS: u32 = 1 << 4; // Receive masked interrupt status
    #[allow(dead_code)]
    pub const MIS_RTMIS: u32 = 1 << 6; // Receive timeout masked interrupt status

    // Interrupt Clear Register (ICR) - Section 3.3.13
    #[allow(dead_code)]
    pub const ICR_RXIC: u32 = 1 << 4; // Receive interrupt clear
    #[allow(dead_code)]
    pub const ICR_RTIC: u32 = 1 << 6; // Receive timeout interrupt clear
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
}

impl fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Handle UART receive interrupt
///
/// Called by the IRQ handler when a UART interrupt fires.
/// Reads all available bytes from the FIFO and clears the interrupt.
pub fn handle_interrupt() {
    // CRITICAL: Must drain FIFO to prevent interrupt storm!
    // If we only clear interrupt status without reading bytes, UART will immediately
    // re-assert interrupt (data still available), causing infinite IRQ loop.
    //
    // We can't acquire WRITER lock here (shell holds it while polling), so we
    // read and discard bytes. Shell's polling will see empty FIFO and wait.
    // This is safe because shell uses blocking read_byte() which polls FIFO status.
    use core::ptr::{read_volatile, write_volatile};
    const UART_BASE: usize = 0xFE201000;
    const DR_OFFSET: usize = 0x00; // Data Register
    const FR_OFFSET: usize = 0x18; // Flag Register
    const ICR_OFFSET: usize = 0x44; // Interrupt Clear Register
    const FR_RXFE: u32 = 1 << 4; // RX FIFO Empty flag

    // SAFETY: Reading/writing UART MMIO registers is safe because:
    // 1. UART_BASE is the correct address for PL011 on BCM2711
    // 2. FR is read-only status register, DR is data register (read clears FIFO entry)
    // 3. ICR is write-only, writing 1s clears interrupt bits
    // 4. This is called from IRQ context, no other safety constraints
    unsafe {
        // Drain all bytes from RX FIFO to prevent interrupt re-assertion
        while (read_volatile((UART_BASE + FR_OFFSET) as *const u32) & FR_RXFE) == 0 {
            let _ = read_volatile((UART_BASE + DR_OFFSET) as *const u32);
            // Byte discarded - shell will timeout and retry
        }

        // Clear interrupt status bits
        write_volatile((UART_BASE + ICR_OFFSET) as *mut u32, (1 << 4) | (1 << 6));
    }
}

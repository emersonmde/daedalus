use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;

/// PL011 UART base address for Raspberry Pi 4
const UART_BASE: usize = 0xFE20_1000;

lazy_static! {
    pub static ref WRITER: Mutex<UartWriter> = Mutex::new(UartWriter::new());
}

/// PL011 UART register offsets
#[repr(C)]
struct Pl011Registers {
    dr: Volatile<u32>,          // 0x00 - Data Register
    _rsv0: [u32; 5],
    fr: Volatile<u32>,          // 0x18 - Flag Register
    _rsv1: [u32; 2],
    ibrd: Volatile<u32>,        // 0x24 - Integer Baud Rate Divisor
    fbrd: Volatile<u32>,        // 0x28 - Fractional Baud Rate Divisor
    lcrh: Volatile<u32>,        // 0x2C - Line Control Register
    cr: Volatile<u32>,          // 0x30 - Control Register
    _rsv2: [u32; 1],
    imsc: Volatile<u32>,        // 0x38 - Interrupt Mask Set/Clear
    _rsv3: [u32; 2],
    icr: Volatile<u32>,         // 0x44 - Interrupt Clear Register
}

/// UART writer for serial console output
pub struct UartWriter {
    registers: &'static mut Pl011Registers,
    initialized: bool,
}

impl UartWriter {
    /// Create a new UART writer instance
    pub const fn new() -> Self {
        UartWriter {
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
        self.registers.icr.write(0x7FF);

        // Set baud rate divisors for 115200 @ 54 MHz
        // Divisor = 54,000,000 / (16 * 115200) = 29.296875
        // Integer part = 29, Fractional part = 0.296875 * 64 = 19
        self.registers.ibrd.write(29);
        self.registers.fbrd.write(19);

        // Configure line control: 8 bits, FIFO enabled, no parity
        // LCRH = (1<<4) | (1<<5) | (1<<6)
        //      = FIFO enable | 8-bit word length
        self.registers.lcrh.write(0x70);

        // Enable UART, TX, and RX
        // CR = (1<<0) | (1<<8) | (1<<9)
        //    = UART enable | TX enable | RX enable
        self.registers.cr.write(0x301);

        self.initialized = true;
    }

    /// Write a single byte to the UART
    pub fn write_byte(&mut self, byte: u8) {
        if !self.initialized {
            self.init();
        }

        // Wait until transmit FIFO is not full (FR bit 5 = TXFF)
        while (self.registers.fr.read() & (1 << 5)) != 0 {}

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
}

impl fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

# UART PL011

ARM PrimeCell UART (PL011) driver reference for Raspberry Pi 4.

## Hardware Configuration

| Parameter | Value | Notes |
|-----------|-------|-------|
| Base Address | `0xFE201000` | See [Memory Map](memory-map.md) |
| Clock Frequency | 48 MHz | **Pi 4 hardware** (QEMU uses 54 MHz) |
| Target Baud Rate | 115200 | Standard serial console speed |
| Data Format | 8N1 | 8 data bits, no parity, 1 stop bit |
| GPIO Pins | 14 (TXD0), 15 (RXD0) | Must be configured to Alt0 function |

## Register Map

| Register | Offset | Name | Purpose |
|----------|--------|------|---------|
| DR | `0x00` | Data Register | Read/write data bytes |
| FR | `0x18` | Flag Register | Status flags (TXFF, RXFE, BUSY) |
| IBRD | `0x24` | Integer Baud Rate Divisor | Integer part of baud divisor |
| FBRD | `0x28` | Fractional Baud Rate Divisor | Fractional part (6 bits) |
| LCRH | `0x2C` | Line Control | Data format, FIFO enable |
| CR | `0x30` | Control Register | Enable UART, TX, RX |
| IMSC | `0x38` | Interrupt Mask | Mask interrupt sources |
| ICR | `0x44` | Interrupt Clear | Clear pending interrupts |

## GPIO Configuration (Required on Hardware)

Pi 4 firmware doesn't always configure GPIO pins for UART. The driver must explicitly set GPIO 14 and 15 to Alt0 function:

```rust
// Configure GPIO 14 (TXD0) and 15 (RXD0) for UART0
const GPFSEL1: usize = 0xFE200004;  // GPIO Function Select 1

let mut fsel = read_volatile(GPFSEL1 as *const u32);
fsel &= !((0b111 << 12) | (0b111 << 15));  // Clear function bits
fsel |= (0b100 << 12) | (0b100 << 15);     // Set to Alt0
write_volatile(GPFSEL1 as *mut u32, fsel);
```

**GPIO Function Select Encoding:**
- Bits 12-14: GPIO 14 function (000=Input, 001=Output, 100=Alt0/UART0_TXD)
- Bits 15-17: GPIO 15 function (000=Input, 001=Output, 100=Alt0/UART0_RXD)

## Initialization Sequence

```rust
// 1. Configure GPIO pins (see above)

// 2. Disable UART during configuration
UART_CR = 0x0000;
small_delay();  // Hardware needs time to disable

// 3. Mask all interrupts
UART_IMSC = 0x0000;

// 4. Clear pending interrupts
UART_ICR = 0x07FF;
small_delay();  // Let FIFOs flush

// 5. Calculate and set baud rate divisors
// Formula: Clock / (16 × BaudRate) = 48000000 / (16 × 115200) = 26.0416...
UART_IBRD = 26;      // Integer part
UART_FBRD = 3;       // Fractional: int(0.0416 × 64 + 0.5)

// 6. Configure line control (8N1, enable FIFO)
UART_LCRH = (1 << 4) | (1 << 5) | (1 << 6);  // 0x70
// Bit 4: Enable FIFOs
// Bits 5-6: Word length = 8 bits

small_delay();  // Stabilize before enabling

// 7. Enable UART, transmitter, receiver
UART_CR = (1 << 0) | (1 << 8) | (1 << 9);  // 0x301
// Bit 0: UART enable
// Bit 8: Transmit enable
// Bit 9: Receive enable

small_delay();  // Let UART stabilize after enabling
```

**Hardware Stabilization Delays:**
Real hardware requires small delays between configuration steps. A simple delay of ~150 NOPs is sufficient:

```rust
fn small_delay() {
    for _ in 0..150 {
        unsafe { core::arch::asm!("nop", options(nomem, nostack)) };
    }
}
```

These delays are harmless on QEMU but critical for hardware stability.

## Transmit (Polling Mode)

```rust
pub fn write_byte(&mut self, byte: u8) {
    // Wait until transmit FIFO has space
    while (self.registers.fr.read() & (1 << 5)) != 0 {
        // Bit 5 = TXFF (Transmit FIFO Full)
        core::hint::spin_loop();
    }

    // Write byte to data register
    self.registers.dr.write(byte as u32);
}
```

## Receive (Polling Mode)

```rust
pub fn read_byte(&mut self) -> u8 {
    // Wait until receive FIFO has data
    while (self.registers.fr.read() & (1 << 4)) != 0 {
        // Bit 4 = RXFE (Receive FIFO Empty)
        core::hint::spin_loop();
    }

    // Read byte from data register
    (self.registers.dr.read() & 0xFF) as u8
}
```

## Baud Rate Calculation

Formula: `Divisor = Clock / (16 × BaudRate)`

For Pi 4 (54 MHz clock, 115200 baud):
- Divisor = 54,000,000 / (16 × 115,200) = 29.296875
- IBRD (integer) = 29
- FBRD (fractional) = int(0.296875 × 64 + 0.5) = 19

**Note**: Pi 3 uses 48 MHz clock, requiring different divisors (IBRD=26, FBRD=3).

## Synchronization

The UART driver is wrapped in `spin::Mutex<UartDriver>` to allow safe concurrent access from:
- Print macros (`print!`, `println!`)
- Shell input/output
- Future interrupt handlers

See `src/lib.rs` for `WRITER` global definition.

## Known Issues & Quirks

1. **Clock frequency differs from Pi 3**: Pi 4 uses 54 MHz, Pi 3 uses 48 MHz
2. **MMIO base differs from Pi 3**: `0xFE201000` vs `0x3F201000`
3. **Polling only**: Interrupts not yet configured (requires GIC setup)

## Code References

- Implementation: `src/drivers/tty/serial/amba_pl011.rs`
- Print macros: `src/lib.rs` (`print!`, `println!`, `_print`)
- Shell I/O: `src/shell.rs`

## External References

- [PL011 Technical Reference Manual](https://developer.arm.com/documentation/ddi0183/latest/) - Sections 3.2, 3.3, 3.4
- [BCM2711 Peripherals PDF](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) - Section 2 (UART)

## Related Documentation

- [Memory Map](memory-map.md) - MMIO base addresses
- [Boot Sequence](../architecture/boot-sequence.md) - When UART is initialized

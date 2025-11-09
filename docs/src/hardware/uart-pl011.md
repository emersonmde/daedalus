# UART PL011

ARM PrimeCell UART (PL011) driver reference for Raspberry Pi 4.

## Hardware Configuration

| Parameter | Value | Notes |
|-----------|-------|-------|
| Base Address | `0xFE201000` | See [Memory Map](memory-map.md) |
| Clock Frequency | 54 MHz | **Pi 4 specific** (Pi 3 uses 48 MHz) |
| Target Baud Rate | 115200 | Standard serial console speed |
| Data Format | 8N1 | 8 data bits, no parity, 1 stop bit |

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

## Initialization Sequence

```rust
// 1. Disable UART
UART_CR = 0x0000;

// 2. Mask all interrupts
UART_IMSC = 0x0000;

// 3. Clear pending interrupts
UART_ICR = 0x07FF;

// 4. Calculate and set baud rate divisors
// Formula: Clock / (16 × BaudRate) = 54000000 / (16 × 115200) = 29.296875
UART_IBRD = 29;      // Integer part
UART_FBRD = 19;      // Fractional: int(0.296875 × 64 + 0.5)

// 5. Configure line control (8N1, enable FIFO)
UART_LCRH = (1 << 4) | (1 << 5) | (1 << 6);  // 0x70
// Bit 4: Enable FIFOs
// Bits 5-6: Word length = 8 bits

// 6. Enable UART, transmitter, receiver
UART_CR = (1 << 0) | (1 << 8) | (1 << 9);  // 0x301
// Bit 0: UART enable
// Bit 8: Transmit enable
// Bit 9: Receive enable
```

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

- Implementation: `src/drivers/uart.rs`
- Print macros: `src/lib.rs` (`print!`, `println!`, `_print`)
- Shell I/O: `src/shell.rs`

## External References

- [PL011 Technical Reference Manual](https://developer.arm.com/documentation/ddi0183/latest/) - Sections 3.2, 3.3, 3.4
- [BCM2711 Peripherals PDF](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) - Section 2 (UART)

## Related Documentation

- [Memory Map](memory-map.md) - MMIO base addresses
- [Boot Sequence](../architecture/boot-sequence.md) - When UART is initialized

# GPIO

GPIO (General Purpose Input/Output) driver for BCM2711.

## Overview

The BCM2711 provides 58 GPIO pins (GPIO 0-57) for general-purpose digital I/O. Each pin can be configured as input, output, or one of six alternate functions (for hardware peripherals like UART, SPI, I2C, etc.).

**Key Features:**
- 58 GPIO pins (BCM2711 specific - BCM2835 had 54)
- Configurable as input, output, or 6 alternate functions
- Built-in pull-up/pull-down resistors (BCM2711 uses new register mechanism)
- 3.3V logic levels (NOT 5V tolerant!)
- Fast digital I/O for bit-banging protocols

## Hardware Reference

- **Base Address**: `0xFE200000` (ARM physical address mapping)
- **Datasheet**: [BCM2711 Peripherals PDF](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) Section 5
- **Reference**: [Linux pinctrl-bcm2835.c](https://github.com/torvalds/linux/blob/master/drivers/pinctrl/bcm/pinctrl-bcm2835.c)

## Register Map

All offsets are from `GPIO_BASE = 0xFE200000`.

### Function Select Registers (GPFSEL0-5)

Control pin modes (input, output, alternate functions).

| Register | Offset | Controls Pins | Description |
|----------|--------|---------------|-------------|
| GPFSEL0  | 0x00   | GPIO 0-9      | Function select for pins 0-9 |
| GPFSEL1  | 0x04   | GPIO 10-19    | Function select for pins 10-19 |
| GPFSEL2  | 0x08   | GPIO 20-29    | Function select for pins 20-29 |
| GPFSEL3  | 0x0C   | GPIO 30-39    | Function select for pins 30-39 |
| GPFSEL4  | 0x10   | GPIO 40-49    | Function select for pins 40-49 |
| GPFSEL5  | 0x14   | GPIO 50-57    | Function select for pins 50-57 |

**Format**: Each pin uses 3 bits (10 pins per 32-bit register).

**Function Codes:**
- `000` (0) - Input
- `001` (1) - Output
- `100` (4) - Alternate Function 0
- `101` (5) - Alternate Function 1
- `110` (6) - Alternate Function 2
- `111` (7) - Alternate Function 3
- `011` (3) - Alternate Function 4
- `010` (2) - Alternate Function 5

### Output Set Registers (GPSET0-1)

Set pins HIGH (write-only, reads return 0).

| Register | Offset | Controls Pins | Description |
|----------|--------|---------------|-------------|
| GPSET0   | 0x1C   | GPIO 0-31     | Write 1 to bit N to set GPIO N high |
| GPSET1   | 0x20   | GPIO 32-57    | Write 1 to bit (N-32) to set GPIO N high |

**Usage**: Writing 1 to a bit sets the corresponding pin HIGH. Writing 0 has no effect.

### Output Clear Registers (GPCLR0-1)

Set pins LOW (write-only, reads return 0).

| Register | Offset | Controls Pins | Description |
|----------|--------|---------------|-------------|
| GPCLR0   | 0x28   | GPIO 0-31     | Write 1 to bit N to set GPIO N low |
| GPCLR1   | 0x2C   | GPIO 32-57    | Write 1 to bit (N-32) to set GPIO N low |

**Usage**: Writing 1 to a bit sets the corresponding pin LOW. Writing 0 has no effect.

### Pin Level Registers (GPLEV0-1)

Read current pin state (read-only).

| Register | Offset | Controls Pins | Description |
|----------|--------|---------------|-------------|
| GPLEV0   | 0x34   | GPIO 0-31     | Read bit N to get GPIO N level |
| GPLEV1   | 0x38   | GPIO 32-57    | Read bit (N-32) to get GPIO N level |

**Usage**: Returns actual pin voltage level (0 = low, 1 = high) regardless of pin mode.

### Pull-up/down Control Registers (BCM2711 Only!)

**IMPORTANT**: BCM2711 uses a completely different mechanism than BCM2835/BCM2836/BCM2837!

The old `GPPUD` and `GPPUDCLK` registers are **not connected** on BCM2711. Use these instead:

| Register | Offset | Controls Pins | Description |
|----------|--------|---------------|-------------|
| GPIO_PUP_PDN_CNTRL_REG0 | 0xE4 | GPIO 0-15  | Pull control for pins 0-15 |
| GPIO_PUP_PDN_CNTRL_REG1 | 0xE8 | GPIO 16-31 | Pull control for pins 16-31 |
| GPIO_PUP_PDN_CNTRL_REG2 | 0xEC | GPIO 32-47 | Pull control for pins 32-47 |
| GPIO_PUP_PDN_CNTRL_REG3 | 0xF0 | GPIO 48-57 | Pull control for pins 48-57 |

**Format**: Each pin uses 2 bits (16 pins per register).

**Pull Codes:**
- `00` (0) - No pull resistor
- `01` (1) - Pull-up resistor enabled
- `10` (2) - Pull-down resistor enabled
- `11` (3) - Reserved

**Example**: To enable pull-up on GPIO 5 (in REG0, bits 10-11):
```rust
let reg = gpio.read(GPIO_PUP_PDN_CNTRL_REG0);
gpio.write(GPIO_PUP_PDN_CNTRL_REG0, (reg & !(0x3 << 10)) | (0x1 << 10));
```

## Reset State

At power-on/reset:
- **All pins configured as INPUT** (GPFSEL = 0x0)
- **All pins have PULL-DOWN enabled** (GPIO_PUP_PDN_CNTRL = 0x2 for each pin)
- Except: Pins used by firmware (UART, I2C) may be configured differently

## Common GPIO Pins

On Raspberry Pi 4, common GPIO usage:

| GPIO | Alt Func | Common Use |
|------|----------|------------|
| 14   | ALT0     | UART0 TXD (console) |
| 15   | ALT0     | UART0 RXD (console) |
| 2    | ALT0     | I2C1 SDA (HAT EEPROM) |
| 3    | ALT0     | I2C1 SCL (HAT EEPROM) |
| 42   | -        | Activity LED (active low) |
| 18   | ALT5     | PWM0 (audio, servo control) |

**Warning**: Avoid GPIOs 0-8 (used for SD card boot), and GPIOs 14-15 if using serial console.

## Electrical Characteristics

- **Logic Levels**: 3.3V (HIGH = 3.3V, LOW = 0V)
- **Absolute Maximum**: 3.6V (do NOT connect 5V signals directly!)
- **Pull Resistors**: ~50-60kΩ (exact value varies)
- **Drive Strength**: Configurable, default ~8mA
- **Maximum Current**: 16mA per pin, 50mA total for all GPIO

## Usage Pattern

Typical sequence for GPIO operations:

**1. Configure as Output:**
```rust
// Set GPIO 42 (Activity LED) as output
gpio.set_function(42, Function::Output);
gpio.set_pull(42, Pull::None);  // LEDs don't need pull resistors
```

**2. Set Output HIGH/LOW:**
```rust
gpio.set(42);    // Turn LED on
gpio.clear(42);  // Turn LED off
```

**3. Configure as Input:**
```rust
// Set GPIO 17 (button) as input with pull-up
gpio.set_function(17, Function::Input);
gpio.set_pull(17, Pull::Up);  // Button to ground, pull-up holds high
```

**4. Read Input:**
```rust
let pressed = !gpio.read(17);  // Active low (button pulls to ground)
```

## Implementation Notes

**Function Select Calculation:**
- Register index: `pin / 10`
- Bit offset: `(pin % 10) * 3`
- Mask: `0b111 << bit_offset`

**Output Set/Clear Calculation:**
- Register index: `pin / 32`
- Bit offset: `pin % 32`
- Mask: `1 << bit_offset`

**Pull Control Calculation (BCM2711):**
- Register index: `pin / 16`
- Bit offset: `(pin % 16) * 2`
- Mask: `0b11 << bit_offset`

## References

- [BCM2711 Peripherals](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) - Section 5 (GPIO)
- [Linux Kernel Driver](https://github.com/torvalds/linux/blob/master/drivers/pinctrl/bcm/pinctrl-bcm2835.c)
- [BCM2711 Pull-up Changes](https://patchwork.ozlabs.org/patch/1134735/) - Kernel patch explaining BCM2711 differences

## Related Documentation

- [Memory Map](memory-map.md) - GPIO base address
- [Raspberry Pi Documentation](../references/raspberry-pi.md) - Pin assignments

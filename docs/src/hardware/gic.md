# GIC-400 Interrupt Controller

**Status**: ✅ Implemented (Milestone #9 complete)

ARM Generic Interrupt Controller v2 (GIC-400) driver for interrupt handling on BCM2711.

## Overview

The GIC-400 is a centralized interrupt controller that:
- Manages up to 1020 interrupt sources
- Routes interrupts to specific CPU cores
- Supports interrupt prioritization and masking
- Provides acknowledge/end-of-interrupt protocol

DaedalusOS uses the GIC for interrupt-driven I/O, starting with UART receive interrupts.

## Hardware Configuration

### Base Addresses

| Component | Address | Purpose |
|-----------|---------|---------|
| **GIC Distributor (GICD)** | `0xFF841000` | Global interrupt configuration |
| **GIC CPU Interface (GICC)** | `0xFF842000` | Per-CPU interrupt handling |

**Source**: BCM2711 ARM Peripherals, Section 6

### Important Note: enable_gic Required

The GIC must be enabled by the firmware. Add to `config.txt`:

```
enable_gic=1
```

Without this setting, the GIC will not function in bare metal mode on Pi 4.

## Interrupt IDs

Interrupt numbering follows ARM GIC-400 specification:

| Range | Type | Description |
|-------|------|-------------|
| 0-15 | SGI | Software Generated Interrupts (inter-core communication) |
| 16-31 | PPI | Private Peripheral Interrupts (per-CPU timers, etc.) |
| 32-1019 | SPI | Shared Peripheral Interrupts (UART, GPIO, etc.) |
| 1020-1023 | Special | Reserved/spurious interrupt IDs |

### BCM2711 Peripheral Interrupts

| Peripheral | Device Tree ID | Actual ID | Type |
|------------|---------------|-----------|------|
| UART0 (PL011) | GIC_SPI 121 | 153 | Level, active high |

**Note**: SPI IDs in device tree are offset by +32 to get the actual GIC interrupt ID.

**Source**: Linux device tree `arch/arm/boot/dts/broadcom/bcm2711.dtsi`

## Architecture

The GIC has two main components:

### Distributor (GICD)

Manages global interrupt state:
- Enable/disable individual interrupts
- Set interrupt priorities (0 = highest, 255 = lowest)
- Configure trigger type (level-sensitive or edge-triggered)
- Route interrupts to specific CPUs

Key registers:
- `GICD_CTLR` (0x000): Enable/disable distributor
- `GICD_TYPER` (0x004): Reports number of interrupt lines
- `GICD_ISENABLERn` (0x100+): Enable interrupts (set-enable)
- `GICD_ICENABLERn` (0x180+): Disable interrupts (clear-enable)
- `GICD_IPRIORITYRn` (0x400+): Set interrupt priorities
- `GICD_ITARGETSRn` (0x800+): Route to CPUs
- `GICD_ICFGRn` (0xC00+): Configure trigger type

### CPU Interface (GICC)

Per-CPU interrupt handling:
- Acknowledge pending interrupts
- Signal end-of-interrupt (EOI)
- Configure priority masking

Key registers:
- `GICC_CTLR` (0x000): Enable/disable CPU interface
- `GICC_PMR` (0x004): Priority mask (accept only interrupts with priority higher than this)
- `GICC_IAR` (0x00C): Interrupt acknowledge (read to get pending interrupt ID)
- `GICC_EOIR` (0x010): End of interrupt (write interrupt ID when done)

## Initialization Sequence

The GIC is initialized in `drivers::gic::Gic::init()`:

1. **Disable distributor** while configuring
2. **Read GICD_TYPER** to get number of interrupt lines
3. **Configure all SPIs** (ID >= 32):
   - Priority: 0xA0 (medium)
   - Target: CPU 0
   - Trigger: Level-sensitive (default for BCM2711 peripherals)
4. **Enable distributor** (both Group 0 and Group 1)
5. **Configure CPU interface**:
   - Priority mask: 0xFF (accept all)
   - Binary point: 0 (all priority bits for preemption)
   - Enable both interrupt groups

## Interrupt Flow

### Enabling an Interrupt

```rust
// Enable UART0 interrupt in GIC
let gic = drivers::gic::GIC.lock();
gic.enable_interrupt(drivers::gic::irq::UART0); // ID 153

// Enable RX interrupt in UART hardware
drivers::uart::WRITER.lock().enable_rx_interrupt();

// Unmask IRQs at CPU level (clear DAIF.I bit)
enable_irqs();
```

### Handling an Interrupt

When an interrupt fires:

1. **CPU takes IRQ exception** → jumps to vector table offset 0x280
2. **Assembly stub** saves context → calls `exception_handler_el1_spx`
3. **Rust handler** calls `handle_irq()`:
   ```rust
   let int_id = gic.acknowledge_interrupt(); // Read GICC_IAR
   // Route to peripheral handler based on int_id
   gic.end_of_interrupt(int_id); // Write GICC_EOIR
   ```
4. **Assembly stub** restores context → executes `eret`

### Priority and Nesting

Current configuration:
- **Priority mask**: 0xFF (lowest priority, accept all interrupts)
- **Binary point**: 0 (all 8 priority bits used for preemption)
- **UART priority**: 0xA0 (medium, higher value = lower priority)

Nested interrupts are **not currently supported** (DAIF.I is set while handling IRQs).

## Implementation Details

**Location**: `src/drivers/irqchip/gic_v2.rs` (356 lines)

Key functions:
- `Gic::init()` - Initialize GIC hardware
- `Gic::enable_interrupt(int_id)` - Enable specific interrupt
- `Gic::disable_interrupt(int_id)` - Disable specific interrupt
- `Gic::acknowledge_interrupt()` - Get pending interrupt ID
- `Gic::end_of_interrupt(int_id)` - Signal completion

All register access uses volatile reads/writes to prevent compiler optimization.

## Testing

The GIC is initialized during kernel startup and tested by:
1. Enabling UART RX interrupts
2. Typing characters in QEMU console
3. Verifying interrupt handler is called (characters are echoed)

## Current Limitations

1. **Single CPU only** - Interrupts routed to CPU 0
2. **No interrupt nesting** - IRQs disabled during handler execution
3. **Level-sensitive only** - Edge-triggered mode not tested
4. **No SGI/PPI support** - Only SPIs (peripheral interrupts) configured

## Future Enhancements

Potential improvements for later milestones:

- **Multi-core support** (Phase 3):
  - Route interrupts to specific CPUs
  - Use SGIs for inter-processor communication
  - Implement per-CPU local timer interrupts (PPIs)

- **Priority-based preemption** (Phase 3):
  - Allow higher-priority interrupts to preempt lower-priority handlers
  - Configure binary point for priority grouping

- **Edge-triggered interrupts**:
  - Support GPIO interrupts (rising/falling edge)
  - Test edge-triggered configuration

- **Interrupt statistics**:
  - Track interrupt counts per source
  - Measure interrupt latency

## Code References

- **GIC driver**: `src/drivers/irqchip/gic_v2.rs`
- **IRQ handler**: `src/arch/aarch64/exceptions.rs` (`handle_irq()`)
- **UART interrupt**: `src/drivers/tty/serial/amba_pl011.rs` (`handle_interrupt()`)
- **Initialization**: `src/lib.rs` (`init()`)

## External References

- **ARM GIC-400 Specification**: [IHI0069 (GIC Architecture Spec)](https://developer.arm.com/documentation/ihi0069/latest/)
  - Section 2: Programmer's model
  - Section 3: Distributor registers
  - Section 4: CPU Interface registers
  - Section 5: Interrupt configuration

- **BCM2711 Documentation**: [BCM2711 ARM Peripherals](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf)
  - Section 6: Interrupt controller

- **Linux Device Tree**: [bcm2711.dtsi](https://github.com/raspberrypi/linux/blob/rpi-6.6.y/arch/arm/boot/dts/broadcom/bcm2711.dtsi)
  - Interrupt IDs for BCM2711 peripherals

## Related Documentation

- [Memory Map](memory-map.md) - GIC base addresses
- [ARM Documentation](../references/arm.md) - GIC specification links
- [Exception Handling](../architecture/exceptions.md) - IRQ exception flow
- [UART Driver](uart-pl011.md) - Interrupt-driven I/O example

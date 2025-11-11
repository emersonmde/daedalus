# System Timer

The BCM2711 System Timer provides a stable 64-bit free-running counter for timing, delays, and scheduling.

## Overview

The System Timer is a simple but crucial peripheral:
- **64-bit counter** at **1 MHz** (1 microsecond per tick)
- **Cannot be stopped** - always running
- **Cannot be reset** - counter value is read-only
- **Hardware guarantees**: Runs at 1 MHz regardless of CPU/GPU clock speeds
- **Overflow**: Won't wrap for ~584,942 years (2^64 microseconds)

This makes it ideal for:
- Accurate microsecond delays
- Performance measurement
- Scheduler tick source (future)
- Timestamps and uptime tracking

## Hardware Characteristics

| Property | Value |
|----------|-------|
| Base Address (ARM) | `0xFE003000` |
| Bus Address | `0x7E003000` (in datasheets) |
| Counter Width | 64-bit |
| Frequency | 1 MHz (fixed) |
| Resolution | 1 microsecond |
| Compare Channels | 4 (C0-C3) |
| ARM-Available Channels | 2 (C1, C3) |
| GPU-Reserved Channels | 2 (C0, C2) |

## Register Map

The System Timer has 8 registers:

| Offset | Name | Width | Access | Description |
|--------|------|-------|--------|-------------|
| +0x00 | CS | 32-bit | R/W | Control/Status (interrupt flags) |
| +0x04 | CLO | 32-bit | R | Counter Lower 32 bits |
| +0x08 | CHI | 32-bit | R | Counter Higher 32 bits |
| +0x0C | C0 | 32-bit | R/W | Compare 0 (used by GPU firmware) |
| +0x10 | C1 | 32-bit | R/W | Compare 1 (available for ARM) |
| +0x14 | C2 | 32-bit | R/W | Compare 2 (used by GPU firmware) |
| +0x18 | C3 | 32-bit | R/W | Compare 3 (available for ARM) |

### CS Register (Control/Status)

The CS register contains interrupt match flags:

| Bit | Name | Description |
|-----|------|-------------|
| 0 | M0 | Timer 0 match detected (GPU) |
| 1 | M1 | Timer 1 match detected (ARM available) |
| 2 | M2 | Timer 2 match detected (GPU) |
| 3 | M3 | Timer 3 match detected (ARM available) |
| 31:4 | - | Reserved |

Write 1 to a bit to clear the corresponding interrupt flag.

### Counter Registers (CLO/CHI)

The 64-bit counter is split across two 32-bit registers:
- **CLO**: Lower 32 bits (bits 31:0)
- **CHI**: Upper 32 bits (bits 63:32)

**Reading the 64-bit counter safely**:
1. Read CHI
2. Read CLO
3. Read CHI again
4. If CHI changed, use the new CHI value

This handles the rare case where CLO rolls over between reads.

### Compare Registers (C0-C3)

Each compare register can trigger an interrupt when the lower 32 bits of the counter match:
- When `counter[31:0] == Cx`, bit Mx in CS is set
- GPU firmware uses C0 and C2 for its own purposes
- ARM can safely use C1 and C3

**Note**: Phase 2 (current) only uses the counter for delays. Interrupts (Milestone #9) will use C1/C3.

## Usage Example

```rust
use daedalus::drivers::timer::SystemTimer;

// Get current timestamp in microseconds
let start = SystemTimer::timestamp_us();

// Delay for 1 millisecond
SystemTimer::delay_ms(1);

// Measure elapsed time
let end = SystemTimer::timestamp_us();
let elapsed = end - start;
println!("Operation took {} microseconds", elapsed);

// Get uptime in seconds
let uptime = SystemTimer::uptime_seconds();
println!("System has been running for {} seconds", uptime);
```

## Implementation Details

### Delay Functions

The driver provides two delay functions:
- `delay_us(n)` - Busy-wait for `n` microseconds
- `delay_ms(n)` - Busy-wait for `n` milliseconds (calls `delay_us(n * 1000)`)

**Accuracy**:
- Resolution: 1 microsecond
- Overhead: ~2-5 microseconds per call
- For delays < 10μs, actual delay may be longer due to overhead

**Implementation**: Simple busy-wait loop that polls the counter.

### Wrap-Around Handling

The 64-bit counter will eventually wrap (after ~584,942 years), though this is unlikely in practice. The delay functions handle wrap-around using `wrapping_add`:

```rust
let start = SystemTimer::read_counter();
let target = start.wrapping_add(microseconds);

if target < start {
    // Wrapped - wait for counter to wrap first
    while SystemTimer::read_counter() >= start {}
}
while SystemTimer::read_counter() < target {}
```

## Shell Commands

The `uptime` command uses the System Timer:

```
daedalus> uptime
Uptime: 5 minutes, 32 seconds
  (332451829 microseconds)
```

## Performance Characteristics

| Operation | Typical Time | Notes |
|-----------|--------------|-------|
| `read_counter()` | ~100 ns | 2 volatile reads + comparison |
| `timestamp_us()` | ~100 ns | Alias for `read_counter()` |
| `delay_us(1)` | ~3-6 μs | Minimum realistic delay |
| `delay_us(100)` | ~100-105 μs | < 5% overhead |
| `delay_ms(1)` | ~1000-1005 μs | < 1% overhead |

Measurements taken in QEMU on Apple M1 host. Real hardware may differ slightly.

## References

### External Documentation

- **BCM2711 ARM Peripherals**: [Section 10 (System Timer)](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf)
- **OSDev Wiki**: [BCM System Timer](https://wiki.osdev.org/BCM_System_Timer)
- **Embedded Xinu**: [BCM2835 System Timer](https://embedded-xinu.readthedocs.io/en/latest/arm/rpi/BCM2835-System-Timer.html)

### Code References

- **Driver**: `src/drivers/clocksource/bcm2711.rs`
- **Shell command**: `src/shell.rs` (uptime command)
- **Tests**: `src/drivers/clocksource/bcm2711.rs` (6 tests: counter, delays, monotonicity)

## Related Documentation

- [Memory Map](memory-map.md) - Timer base address
- [GIC-400 Interrupt Controller](gic.md) - Timer interrupt routing (Phase 2, Milestone #9)
- [Roadmap](../roadmap.md) - Milestone #8 completion

## Future Enhancements

Planned for Milestone #9 (GIC-400 Setup):
- Configure timer compare interrupts (C1 or C3)
- Replace busy-wait delays with interrupt-driven timing
- Scheduler tick for preemptive multitasking (Phase 3)

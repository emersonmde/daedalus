# GIC-400 Interrupt Controller

**Status**: Not yet implemented (Phase 3 milestone)

ARM Generic Interrupt Controller v2 (GIC-400) driver for interrupt handling.

## Planned Implementation

Required for:
- UART interrupt-driven I/O (instead of polling)
- Timer interrupts for scheduler
- Multi-core interrupt routing

## Hardware Reference

- **GIC Distributor Base**: `0xFF841000`
- **Datasheet**: [BCM2711 Peripherals PDF](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) Section 6
- **ARM Spec**: [GIC-400 Architecture Specification](https://developer.arm.com/documentation/ihi0069/latest/)

## Current State

Interrupts are masked (DAIF bits set). All I/O uses polling.

## Related Documentation

- [Memory Map](memory-map.md) - GIC base address
- [ARM Documentation](../references/arm.md) - GIC specification
- [Exception Handling](../architecture/exceptions.md) - Exception infrastructure

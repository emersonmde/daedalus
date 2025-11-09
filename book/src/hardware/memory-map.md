# Memory Map

Physical address layout for Raspberry Pi 4 (BCM2711).

## Address Ranges

| Start | End | Size | Purpose |
|-------|-----|------|---------|
| `0x00000000` | `0x3FFFFFFF` | 1 GB | DRAM (varies by Pi 4 model: 1/2/4/8 GB) |
| `0x00080000` | - | - | Kernel load address (firmware entry point) |
| `0xFE000000` | `0xFF800000` | ~24 MB | MMIO peripherals window |

## MMIO Base Address

**CRITICAL**: Pi 4 uses `0xFE000000` for ARM CPU peripheral access.

### Why Multiple Addresses Exist

The BCM2711 chip supports different address mappings:
- **ARM physical access**: `0xFE000000` ← **Use this for bare-metal**
- **Bus addressing**: `0x7E000000` (appears in datasheets)
- **Pi 3 legacy**: `0x3F000000` (BCM2837, not applicable to Pi 4)

When reading BCM2711 documentation showing bus addresses (`0x7E00xxxx`), translate to ARM physical by replacing `0x7E` with `0xFE`.

## Key Peripheral Addresses

| Peripheral | Base Address | Datasheet Reference |
|------------|--------------|---------------------|
| UART0 (PL011) | `0xFE201000` | BCM2711 §2.1 |
| GPIO | `0xFE200000` | BCM2711 §5.2 |
| System Timer | `0xFE003000` | BCM2711 §10.2 |
| GIC Distributor | `0xFF841000` | BCM2711 §6.1 |
| Mailbox | `0xFE00B880` | BCM2711 §1.3 |

## Memory Allocation (Current)

### Kernel Image
- Loaded at `0x00080000` by firmware
- Size: ~100 KB (debug), ~50 KB (release)

### Heap
- Defined by `__heap_start` and `__heap_end` symbols in [linker script](../architecture/linker-script.md)
- Size: 8 MB reserved
- **Not yet used** - allocator not implemented (Phase 2 milestone)
- Location: After BSS section, aligned to 16 bytes

### Stack
- Defined by `_stack_start` symbol in [linker script](../architecture/linker-script.md)
- Size: 2 MB (currently only core 0 uses it)
- Location: After heap, grows downward
- Alignment: 16 bytes (ARM AAPCS requirement)

## Code References

- Linker script: `linker.ld`
- UART base constant: `src/drivers/uart.rs` (`UART_BASE`)
- Memory map table: `PROJECT.md` Section 2

## External References

- [BCM2711 ARM Peripherals PDF](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) - Section 1.2 (address map)
- [ARM Cortex-A72 TRM](https://developer.arm.com/documentation/100095/0003) - Section 4.3 (memory attributes)

## Related Documentation

- [UART Hardware Reference](uart-pl011.md) - UART peripheral details
- [Linker Script](../architecture/linker-script.md) - Section placement and symbols

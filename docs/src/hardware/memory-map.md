# Memory Map

Physical and virtual address layout for Raspberry Pi 4 (BCM2711).

## Virtual Memory

**Status**: MMU enabled with identity mapping (VA = PA)

After kernel initialization, the MMU is active with:
- 39-bit virtual address space (512 GB)
- 4 KB page granule, 2 MB block mappings
- Identity mapping: virtual address equals physical address

See [MMU & Paging](../architecture/mmu-paging.md) for details.

## Address Ranges

| Start | End | Size | Purpose |
|-------|-----|------|---------|
| `0x00000000` | `0x3FFFFFFF` | 1 GB | DRAM (varies by Pi 4 model: 1/2/4/8 GB) |
| `0x00080000` | - | - | Kernel load address (firmware entry point) |
| `0xC0000000` | `0xFFFFFFFF` | ~1 GB | Reserved (future expansion) |
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

All MMIO regions are mapped as device memory (non-cacheable, strictly ordered).

| Peripheral | Base Address | Datasheet Reference |
|------------|--------------|---------------------|
| UART0 (PL011) | `0xFE201000` | BCM2711 §2.1 |
| GPIO | `0xFE200000` | BCM2711 §5.2 |
| System Timer | `0xFE003000` | BCM2711 §10.2 |
| GIC-400 Distributor (GICD) | `0xFF841000` | BCM2711 §6.1 |
| GIC-400 CPU Interface (GICC) | `0xFF842000` | BCM2711 §6.1 |
| Mailbox | `0xFE00B880` | BCM2711 §1.3 |

## Kernel Memory Layout

Detailed layout defined in `linker.ld`:

| Section | Address | Size | Description |
|---------|---------|------|-------------|
| `.text.boot` | `0x00080000` | ~256 B | Assembly entry point |
| `.text.exceptions` | `0x00080800` | 2 KB | Exception vector table (2KB aligned) |
| `.text` | `0x00081000+` | ~50-100 KB | Rust kernel code |
| `.rodata` | After .text | ~10-20 KB | Read-only data, string literals |
| `.data` | After .rodata | ~1-5 KB | Initialized global variables |
| `.bss` | After .data | ~20-30 KB | Zero-initialized data + page tables |
| Heap | `__heap_start` | 8 MB | Dynamic allocations (String, Vec, etc.) |
| Stack | `_stack_start` | 2 MB | Call stack (grows downward) |

### Kernel Image
- Loaded at `0x00080000` by firmware
- Total size: ~100 KB (debug), ~50 KB (release)
- Entry point: `_start` in `src/arch/aarch64/boot.s`

### Page Tables (in .bss)
- `L1_TABLE`: 4 KB (512 entries, 1 GB per entry)
- `L2_TABLE_LOW`: 4 KB (maps 0-1 GB normal memory)
- `L2_TABLE_MMIO`: 4 KB (maps 3-4 GB device memory)
- Total: 12 KB for translation tables

### Heap
- Defined by `__heap_start` and `__heap_end` symbols in [linker script](../architecture/linker-script.md)
- Size: 8 MB reserved
- **Active**: Bump allocator implemented (Phase 2 complete)
- Location: After BSS section, aligned to 16 bytes
- Used for: `String`, `Vec`, shell command history, dynamic allocations

### Stack
- Defined by `_stack_start` symbol in [linker script](../architecture/linker-script.md)
- Size: 2 MB (currently only core 0 uses it)
- Location: After heap, grows downward toward heap
- Alignment: 16 bytes (ARM AAPCS requirement)
- Future: Per-core stacks when multi-core support is added

## Memory Attributes

After MMU initialization:

| Region | Type | Cacheable | Shareable | Permissions |
|--------|------|-----------|-----------|-------------|
| 0x00000000-0x3FFFFFFF | Normal | Yes (WB) | Inner | EL1 RW |
| 0xFE000000-0xFF800000 | Device | No | No | EL1 RW |

**Normal Memory** (kernel code/data):
- Write-Back, Read/Write-Allocate caching
- Inner Shareable for multi-core coherency
- ~100x faster than uncached access

**Device Memory** (MMIO):
- Device-nGnRnE (non-Gathering, non-Reordering, no Early-ack)
- Strictly ordered, every access reaches hardware
- Required for correct peripheral operation

See [MMU & Paging](../architecture/mmu-paging.md) for MAIR_EL1 configuration details.

## Future Memory Regions

Planned for future milestones:

| Address Range | Purpose | Milestone |
|---------------|---------|-----------|
| Higher-half kernel | Kernel at 0xFFFF_8000_0000_0000+ | Phase 5-6 |
| Per-core stacks | 2 MB × 4 cores | Phase 3 (Multi-core) |
| Userspace | Lower 256 TB for user programs | Phase 4 (EL0 Userspace) |

## Code References

- Linker script: `linker.ld`
- Page tables: `src/arch/aarch64/mmu.rs` (L1_TABLE, L2_TABLE_*)
- UART base: `src/drivers/uart.rs` (`UART_BASE`)
- GIC base: `src/drivers/gic.rs` (`GICD_BASE`, `GICC_BASE`)

## External References

- <https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf> - Section 1.2 (address map)
- <https://developer.arm.com/documentation/100095/0003> - Section 4.3 (memory attributes)

## Related Documentation

- [MMU & Paging](../architecture/mmu-paging.md) - Virtual memory configuration
- [Boot Sequence](../architecture/boot-sequence.md) - Memory initialization
- [UART Hardware Reference](uart-pl011.md) - UART peripheral details
- [Linker Script](../architecture/linker-script.md) - Section placement and symbols
- [GIC Interrupts](gic.md) - Interrupt controller addresses

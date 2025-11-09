# MMU & Paging

**Status**: Not yet implemented (Phase 2/3 milestone)

Memory Management Unit and virtual memory configuration.

## Planned Implementation

The MMU enables:
- Virtual memory mapping
- Memory protection
- Cache control
- Userspace isolation (Phase 4)

## Current State

- MMU disabled (identity mapping)
- All addresses are physical
- No memory protection

## Future Design

### Translation Tables

- **Identity map**: First 64 MB (kernel + DRAM)
- **MMIO window**: Device memory at `0xFE000000-0xFF800000`
- **Higher-half kernel**: Map kernel to high addresses (optional)

### Page Size

- **4 KiB pages**: Standard granularity
- **2 MB sections**: For large mappings (kernel, MMIO)

## ARM References

- [ARM Cortex-A72 TRM](https://developer.arm.com/documentation/100095/0003) - Section 8 (MMU)
- [ARMv8-A ISA](https://developer.arm.com/documentation/ddi0602/2024-12) - Section D4 (Virtual Memory)

## Related Documentation

- [Memory Map](../hardware/memory-map.md) - Physical address layout
- [Linker Script](linker-script.md) - Section alignment for paging
- [ARM Documentation](../references/arm.md) - Translation table format

# MMU & Paging

**Status**: ✅ Implemented (Phase 3, Milestone #10)

Memory Management Unit and virtual memory configuration for DaedalusOS.

## Overview

The MMU provides:
- **Virtual memory**: 39-bit address space (512 GB)
- **Memory protection**: Separate attributes for kernel and MMIO regions
- **Cache control**: Cacheable normal memory, non-cacheable device memory
- **Foundation for userspace**: Ready for EL0 isolation (Phase 4)

## Implementation Details

### Address Space Configuration

- **Virtual address size**: 39 bits (512 GB)
- **Page granule**: 4 KB
- **Translation levels**: 3 (L1, L2, L3)
- **Mapping strategy**: Identity mapping (VA = PA)

**Why these choices?**
- 39-bit VA requires only 3 page table levels (vs 4 for 48-bit)
- 4 KB pages are universally supported and efficient
- Identity mapping simplifies boot and hardware access

### Translation Table Structure

```
L1 Table (512 entries, each covers 1 GB):
  ├─ Entry 0 → L2_TABLE_LOW (0-1 GB region)
  ├─ Entry 1-2 → Unmapped
  └─ Entry 3 → L2_TABLE_MMIO (3-4 GB region)

L2_TABLE_LOW (512 entries, each covers 2 MB):
  ├─ Entry 0-511 → 2 MB blocks, Normal memory (0-1 GB)
  └─ Attributes: Cacheable, Inner Shareable, EL1 RW

L2_TABLE_MMIO (512 entries, each covers 2 MB):
  ├─ Entry 0-511 → 2 MB blocks, Device memory (3-4 GB)
  └─ Attributes: Device-nGnRnE, Non-shareable, EL1 RW
```

### Memory Mappings

| Virtual Address | Physical Address | Size | Type | Description |
|----------------|------------------|------|------|-------------|
| `0x00000000-0x3FFFFFFF` | Same (identity) | 1 GB | Normal | Kernel code, data, heap, DRAM |
| `0xFE000000-0xFF800000` | Same (identity) | ~24 MB | Device | MMIO peripherals (UART, GIC, etc.) |

### Memory Attributes (MAIR_EL1)

```
Attr0 (Device):  0x00 = Device-nGnRnE
  - Non-Gathering: Each access is separate
  - Non-Reordering: Access order is preserved
  - No Early-ack: Wait for completion

Attr1 (Normal):  0xFF = Normal, Write-Back, Read/Write-Allocate
  - Inner/Outer cacheable
  - Write-back policy
  - Allocate on read and write
```

**Reference**: ARM ARM Section D4.4.4, Table D4-17

### Translation Control (TCR_EL1)

```
T0SZ     = 25     → 2^(64-25) = 512 GB address space
TG0      = 4 KB   → Page granule size
SH0      = Inner Shareable (for SMP)
ORGN0/IRGN0 = Write-Back Write-Allocate
IPS      = 40-bit → 1 TB physical address support
```

**Reference**: ARM ARM Section D4.2.6, Table D4-11

### System Registers

The MMU uses these AArch64 system registers:

- **SCTLR_EL1**: System Control Register
  - Bit 0 (M): MMU enable
  - Bit 2 (C): Data cache enable
  - Bit 12 (I): Instruction cache enable

- **TTBR0_EL1**: Translation Table Base Register
  - Points to L1 translation table
  - Must be 4 KB aligned

- **MAIR_EL1**: Memory Attribute Indirection Register
  - Defines 8 memory attribute encodings
  - Referenced by page table entries

- **TCR_EL1**: Translation Control Register
  - Configures address space size, granule, cacheability

**Reference**: ARM ARM Section C5.2 (System Registers)

## Initialization Sequence

The MMU is initialized during kernel startup in `src/lib.rs:init()`:

1. **Set up translation tables** (`setup_page_tables`)
   - Initialize L1, L2_LOW, and L2_MMIO tables
   - Create identity mappings for kernel and MMIO

2. **Configure memory attributes** (`MAIR_EL1`)
   - Attr0: Device-nGnRnE for MMIO
   - Attr1: Normal WB for kernel/DRAM

3. **Configure translation control** (`TCR_EL1`)
   - Set address space size (39-bit)
   - Configure granule size (4 KB)
   - Enable caching and shareability

4. **Set translation table base** (`TTBR0_EL1`)
   - Point to L1 table physical address

5. **Enable MMU** (`SCTLR_EL1`)
   - Set M bit (MMU enable)
   - Set C bit (data cache enable)
   - Set I bit (instruction cache enable)

6. **Synchronization barriers**
   - `DSB SY`: Ensure all writes complete
   - `ISB`: Flush instruction pipeline

**Code location**: `src/arch/aarch64/mmu.rs`

## Shell Commands

Use the `mmu` command to inspect MMU status:

```
daedalus> mmu
MMU (Memory Management Unit) Status:

  Status: ENABLED

  Translation Table Base (TTBR0_EL1): 0x00000000000A5000

  Translation Control (TCR_EL1): 0x0000000080803519
    Virtual address size: 39 bits (512 GB)
    Page granule: 4 KB

  Memory Attributes (MAIR_EL1): 0x000000000000FF00
    Attr0 (Device): 0x00 (Device-nGnRnE)
    Attr1 (Normal):  0xFF (Normal WB RW-Allocate)

  Memory Mappings (Identity):
    0x00000000-0x3FFFFFFF → Normal memory (kernel + DRAM)
    0xFE000000-0xFF800000 → Device memory (MMIO)
```

## Design Decisions

### Why Identity Mapping?

We use identity mapping (VA = PA) instead of higher-half kernel because:

1. **Boot simplicity**: No address space switch needed during MMU enablement
2. **No relocation**: Kernel code/data/linker symbols work without modification
3. **Clear debugging**: Virtual address = physical hardware address
4. **Standard for bare-metal**: Easier to reason about hardware access

Future work can add higher-half mapping (e.g., kernel at `0xFFFF_8000_0000_0000+`) without changing MMIO access patterns.

### Why 2 MB Blocks (Not 4 KB Pages)?

We use 2 MB block mappings at L2 instead of 4 KB pages at L3 because:

1. **Fewer TLB entries**: Larger blocks = fewer Translation Lookaside Buffer entries
2. **Simpler page tables**: No need for L3 tables (saves 2 MB per GB mapped)
3. **Sufficient granularity**: We don't need fine-grained protection yet
4. **Performance**: Fewer page table walks

We can add L3 tables later for fine-grained memory protection (e.g., read-only .text, no-execute heap).

## Future Enhancements

### Phase 4: Userspace (EL0)

- Add separate TTBR1_EL1 for kernel space
- Configure EL0 access permissions
- Map user programs with restricted permissions
- Implement copy-on-write for processes

### Phase 5: Fine-Grained Protection

- Add L3 tables for 4 KB page granularity
- Make `.text` section read-only and executable
- Make `.rodata` section read-only
- Make heap/stack non-executable (NX)

### Phase 6: Higher-Half Kernel

- Map kernel to high addresses (`0xFFFF_8000_0000_0000+`)
- Keep MMIO at low addresses (identity mapped)
- Allows full lower address space for userspace

## Debugging

### Common Issues

**MMU doesn't enable (SCTLR_EL1.M = 0)**:
- Check TTBR0_EL1 points to valid page table
- Verify page table entries are valid (descriptor type bits)
- Ensure TCR_EL1 is correctly configured

**Data abort on MMU enable**:
- Check page table covers all accessed addresses
- Verify MAIR_EL1 attributes match page table AttrIndx
- Ensure stack/heap are in mapped regions

**Cache coherency issues**:
- Add DSB/ISB barriers after page table modifications
- Invalidate TLB after changes (`TLBI` instruction)

### Useful ARM Instructions

```
MRS x0, SCTLR_EL1    ; Read system control
MRS x0, TTBR0_EL1    ; Read table base
MRS x0, TCR_EL1      ; Read translation control
MRS x0, MAIR_EL1     ; Read memory attributes
TLBI VMALLE1         ; Invalidate all TLB entries
DC CIVAC, x0         ; Clean and invalidate data cache by VA
```

## ARM References

- <https://developer.arm.com/documentation/100095/0003> - Cortex-A72 TRM Section 8 (MMU)
- <https://developer.arm.com/documentation/ddi0602/2024-12> - ARMv8-A ISA Section D4 (Virtual Memory)
- <https://developer.arm.com/documentation/102376/latest> - Learn the Architecture: Memory Management

## Related Documentation

- [Memory Map](../hardware/memory-map.md) - Physical address layout
- [Linker Script](linker-script.md) - Section alignment for paging
- [ARM Documentation](../references/arm.md) - Translation table format
- [Boot Sequence](boot-sequence.md) - MMU initialization during boot

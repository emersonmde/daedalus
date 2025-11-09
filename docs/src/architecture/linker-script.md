# Linker Script

**File**: `linker.ld`

The linker script controls how the kernel binary is laid out in memory at link time.

## Key Decisions

### Entry Address: 0x80000

Pi 4 firmware loads `kernel8.img` to physical address `0x80000` and jumps there. This is a hardware constraint, not a choice.

**Why this address?**
- Historical: ARM bootloaders have used 0x8000 or 0x80000
- Pi firmware convention: kernel8.img → 64-bit mode → 0x80000
- Well below MMIO window (0xFE000000), plenty of room for DRAM

### Exception Vector Alignment: 2048 bytes

The ARM architecture **requires** exception vector tables be aligned to 2048 bytes (0x800).

**Why?**
- VBAR_EL1 register ignores low 11 bits when setting vector base
- ARM ARM D1.10.2: "aligned to 0x800 (2048 bytes)"
- Linker enforces this: `.text.exceptions : ALIGN(0x800)`

### Memory Layout Order

```
.text.boot        ← First! Firmware jumps to 0x80000
.text.exceptions  ← Exception vectors (aligned to 0x800)
.text             ← Main Rust code
.rodata           ← String literals, const data
.data             ← Initialized globals
.bss              ← Zero-initialized globals
Heap (8 MB)       ← Reserved for Phase 2 allocator
Stack (2 MB)      ← Grows downward from top
```

**Why this order?**
- Boot code must be first (entry point)
- Exceptions need special alignment, easier before main code
- Standard ELF convention: code → rodata → data → bss
- Stack at end makes overflow detection easier (future)

### Heap Size: 8 MB

Currently reserved but unused. Allocator is Phase 2 milestone.

**Why 8 MB?**
- Enough for shell history, command buffers, future features
- Small enough to not waste limited Pi 1 GB RAM
- Can be adjusted based on actual usage

### Stack Size: 2 MB

**Why 2 MB?**
- Conservative estimate for deep call stacks
- Exception handlers save ~264 bytes per exception
- Future: Will split per-core when enabling SMP

## Symbol Exports

The linker script defines symbols that boot code and future allocator use:

| Symbol | Used By | Purpose |
|--------|---------|---------|
| `__bss_start`, `__bss_end` | `boot.s` | Clear BSS loop bounds |
| `__heap_start`, `__heap_end` | Future allocator | Heap memory region |
| `_stack_start` | `boot.s` | Initial stack pointer |

**How they're used:**
- Boot assembly reads these to know where BSS is
- Future allocator reads heap bounds to manage free list
- No runtime overhead - these are compile-time addresses

## Alignment Requirements

- **BSS**: 16-byte aligned (ARM AAPCS calling convention)
- **Stack**: 16-byte aligned (ARM AAPCS requirement for function calls)
- **Heap**: 16-byte aligned (allocation efficiency)
- **Exception vectors**: 2048-byte aligned (ARM architectural requirement)

## Future Changes

### When Adding MMU (Phase 2/3)

Will need to align sections to page boundaries:
- 4 KiB alignment for small pages
- 2 MB alignment for large pages/sections

Example: `. = ALIGN(4096);` before each major section.

### When Adding Multi-Core (Phase 3)

Will need per-core stacks:
```ld
.stack (NOLOAD) : {
    . = ALIGN(16);
    . += (0x200000 * 4);  /* 2 MB × 4 cores */
    _stack_start = .;
}
```

## Debugging Tips

**"Kernel doesn't boot":**
- Check entry address is 0x80000: `readelf -h kernel.elf | grep Entry`
- Verify .text.boot is first: `readelf -S kernel.elf | head -20`

**"Exception handling broken":**
- Check vector alignment: `readelf -S kernel.elf | grep exceptions`
- Must show `ALIGN: 0x800`

**"Stack overflow":**
- Add guards: `__stack_end` symbol for overflow detection
- Reduce stack size or increase in linker script

## Related Documentation

- [Boot Sequence](boot-sequence.md) - How symbols are used during boot
- [Memory Map](../hardware/memory-map.md) - Physical address layout
- [Exception Handling](exceptions.md) - Why vector alignment matters

## External References

- ARM AAPCS (calling convention): Requires 16-byte stack alignment
- ARM ARM D1.10.2: Vector table alignment requirement
- ELF specification: Standard section ordering

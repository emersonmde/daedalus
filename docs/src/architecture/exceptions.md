# Exception Handling

**Status**: Implemented (Milestone #7 complete)

ARMv8-A exception handling with vector table, context save/restore, and register dumps.

## Overview

The exception handling system provides:
- 16-entry exception vector table (aligned to 2048 bytes)
- Full context save/restore (all GPRs + system registers)
- ESR (Exception Syndrome Register) decoding
- FAR (Fault Address Register) reporting
- Complete register dumps on exceptions

## Vector Table Structure

ARM ARM D1.10.2 specifies 16 vectors: 4 exception types × 4 exception levels

```
Offset  | Exception Type      | Exception Level
--------|--------------------|-----------------
0x000   | Synchronous        | Current EL, SP0
0x080   | IRQ                | Current EL, SP0
0x100   | FIQ                | Current EL, SP0
0x180   | SError             | Current EL, SP0
0x200   | Synchronous        | Current EL, SPx  ← Normal kernel exceptions
0x280   | IRQ                | Current EL, SPx
0x300   | FIQ                | Current EL, SPx
0x380   | SError             | Current EL, SPx
0x400   | Synchronous        | Lower EL, AArch64
0x480   | IRQ                | Lower EL, AArch64
0x500   | FIQ                | Lower EL, AArch64
0x580   | SError             | Lower EL, AArch64
0x600   | Synchronous        | Lower EL, AArch32
0x680   | IRQ                | Lower EL, AArch32
0x700   | FIQ                | Lower EL, AArch32
0x780   | SError             | Lower EL, AArch32
```

Each vector is 128 bytes (0x80), aligned to 128-byte boundary.

## Exception Types

- **Synchronous**: Instruction aborts, data aborts, syscalls (SVC), breakpoints (BRK)
- **IRQ**: Normal interrupts (disabled until GIC configured)
- **FIQ**: Fast interrupts (disabled)
- **SError**: Asynchronous system errors

## Context Save/Restore

Assembly macros save all registers to stack-allocated `ExceptionContext`:

```rust
#[repr(C)]
pub struct ExceptionContext {
    // General purpose registers
    x0: u64, x1: u64, ..., x30: u64,

    // System registers
    elr_el1: u64,   // Exception Link Register (return address)
    spsr_el1: u64,  // Saved Program Status Register
}
```

**Size**: 33 registers × 8 bytes = 264 bytes per exception

## ESR Decoding

Exception Syndrome Register (ESR_EL1) bits [31:26] contain exception class:

| EC Value | Exception Class |
|----------|----------------|
| 0x00 | Unknown reason |
| 0x01 | Trapped WFI/WFE |
| 0x07 | SVE/SIMD/FP access |
| 0x15 | SVC (syscall) from AArch64 |
| 0x18 | Trapped MSR/MRS/system instruction |
| 0x20 | Instruction abort from lower EL |
| 0x21 | Instruction abort from same EL |
| 0x24 | Data abort from lower EL |
| 0x25 | Data abort from same EL |
| 0x2C | Floating point exception |
| 0x3C | BRK instruction (breakpoint) |

Full list: 40+ exception classes decoded in `src/arch/aarch64/exceptions.rs`.

## FAR (Fault Address Register)

For memory access exceptions (instruction/data aborts, alignment faults):
- FAR_EL1 contains the faulting virtual address
- FAR_EL2 used when running at EL2 (QEMU)

## Installation

Vector table installed during `daedalus::init()`:

```rust
pub unsafe fn install_vector_table() {
    let vbar_addr = &exception_vector_table as *const _ as u64;

    // Set VBAR_EL1 or VBAR_EL2 based on current exception level
    if current_el() == 2 {
        asm!("msr vbar_el2, {}", in(reg) vbar_addr);
    } else {
        asm!("msr vbar_el1, {}", in(reg) vbar_addr);
    }
}
```

## Exception Flow

1. **Exception occurs** (e.g., BRK instruction, data abort)
2. **CPU jumps** to appropriate vector (e.g., offset 0x200 for synchronous at current EL)
3. **Assembly stub** saves all registers to stack
4. **Rust handler** called with `&ExceptionContext`
5. **Handler prints** exception info: type, ESR, FAR, all registers
6. **Panic** (current behavior - no recovery yet)

## Testing

### Shell Command
```
daedalus> exception
```
Triggers BRK instruction, prints full exception dump.

### Test Suite
```bash
cargo test
```
Runs 25 tests including exception vector installation.

## Known Issues

**EL2 vs EL1 Discrepancy:**
- QEMU boots at EL2, real hardware boots at EL1
- Assembly hardcodes EL1 register saves (ELR_EL1, SPSR_EL1)
- In QEMU: ELR/SPSR show as zero (GPRs and FAR are correct)
- Code checks `current_el()` and reads FAR_EL2 when at EL2

**Future Fix**: Make exception assembly EL-agnostic or drop to EL1 during boot.

## Code References

- Vector table: `src/arch/aarch64/exceptions.s`
- Context struct: `src/arch/aarch64/exceptions.rs`
- ESR decoding: `src/arch/aarch64/exceptions.rs` (`exception_class_str`)
- Installation: `src/arch/aarch64/exceptions.rs` (`install_vector_table`)

## External References

- [ARMv8-A ISA](https://developer.arm.com/documentation/ddi0602/2024-12) - Section D1.10 (exception model)
- [ARM Cortex-A72 TRM](https://developer.arm.com/documentation/100095/0003) - Section 5 (exceptions)

## Related Documentation

- [Boot Sequence](boot-sequence.md) - When vectors are installed
- [ARM Documentation](../references/arm.md) - ESR/FAR register details

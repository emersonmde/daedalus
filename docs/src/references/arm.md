# ARM Documentation

ARM architecture references organized by topic for quick lookup.

## When to Use

Consult when implementing low-level features: exceptions, system registers, MMU, assembly code, or debugging unexpected CPU behavior.

## Core Documentation

### ARM Cortex-A72 Processor (Our CPU)

[Cortex-A72 MPCore Processor Technical Reference Manual](https://developer.arm.com/documentation/100095/0003)

**Key sections:**
- **Section 2**: Functional description and features
- **Section 3.3**: Power management (WFE/WFI instructions for core parking)
- **Section 4**: System control
  - 4.2: System control registers (SCTLR_EL1, CPACR_EL1)
  - 4.3: Memory system (caches, MMU control)
  - 4.4: Exception handling configuration
- **Section 5**: Exceptions and debug
  - 5.2: Exception model
  - 5.3: Exception handling (VBAR_EL1 setup)
- **Section 6**: Caches
  - 6.2: L1 cache (future optimization)
  - 6.3: L2 cache configuration
- **Section 8**: Memory Management Unit
  - 8.2: Translation tables (for Phase 2/3)
  - 8.3: TLB maintenance

### ARMv8-A Instruction Set Architecture

[A-profile A64 Instruction Set Architecture (2024-12)](https://developer.arm.com/documentation/ddi0602/2024-12)

**Key sections:**
- **Section A1**: Instruction encoding and syntax
  - A1.3: Registers (X0-X30, SP, PC)
  - A1.6: Instruction set overview
- **Section C5**: System register descriptions
  - C5.2.7: `MPIDR_EL1` (multiprocessor affinity - for core detection)
  - C5.2.18: `VBAR_EL1` (vector base address - exception table)
  - C5.2.5: `ESR_EL1` (exception syndrome - what caused exception)
  - C5.2.6: `FAR_EL1` (fault address - where memory fault occurred)
  - C5.2.8: `ELR_EL1` (exception link - return address)
  - C5.2.16: `SPSR_EL1` (saved program status)
  - C5.2.14: `SCTLR_EL1` (system control - MMU enable, cache enable)
- **Section D1**: The AArch64 System Level Programmers' Model
  - D1.2: Exception levels (EL0-EL3)
  - D1.10: Exception model and vectors
  - D1.10.2: Vector table layout (16 entries × 128 bytes)
  - D1.11: Exception syndrome register (ESR_EL1 decoding)
- **Section D4**: The AArch64 Virtual Memory System Architecture
  - D4.2: Translation tables (for MMU work)
  - D4.3: Page table format
  - D4.4: Memory attributes and types

**Quick references:**
- [A64 Base Instructions](https://developer.arm.com/documentation/ddi0602/2024-12/Base-Instructions) - Alphabetical instruction list
- [ARMv8-A ISA Overview PDF](https://developer.arm.com//-/media/Arm%20Developer%20Community/PDF/Learn%20the%20Architecture/Armv8-A%20Instruction%20Set%20Architecture.pdf) - Learning guide

### ARM Generic Interrupt Controller

[GIC-400 Architecture Specification](https://developer.arm.com/documentation/ihi0069/latest/)

**Needed for Phase 3 (interrupts)**
- **Section 2**: Programmers' model
- **Section 3**: GIC distributor (GICD) at 0xFF841000
- **Section 4**: CPU interface (GICC)
- **Section 5**: Interrupt configuration

Note: Pi 4 uses GIC-400 (not GIC-500/600 found in newer ARM platforms).

## Usage Patterns

### Implementing Exception Handling

1. Start with **ISA Section D1.10** for exception model overview
2. Check **Cortex-A72 Section 5** for A72-specific details
3. Use **ISA Section C5** for system register bitfields (VBAR_EL1, ESR_EL1, FAR_EL1)
4. Reference **ISA Section D1.11** for ESR decoding

### Debugging Unexpected Behavior

1. Check **Cortex-A72 Section 4** for reset state and defaults
2. Verify exception level in **ISA Section D1.2**
3. Review system register access permissions in **ISA Section C5**
4. Compare QEMU vs hardware behavior (QEMU boots EL2, hardware boots EL1)

### Writing Assembly Code

1. Use **ISA Section A1** for instruction syntax
2. Check **A64 Base Instructions** for specific instruction details
3. Verify register usage in **ISA Section A1.3**
4. Consult **Cortex-A72 Section 3** for core-specific features

## Common Pitfalls

### Exception Level Confusion
- **QEMU boots at EL2**, real Pi 4 hardware boots at EL1
- Affects which registers are accessible
- Some EL1 registers (ELR_EL1, SPSR_EL1) may show zero in QEMU
- Solution: Check current EL and use appropriate registers

### Register Access
- System registers have specific access requirements per exception level
- **Read ISA Section C5** for each register's access permissions
- Accessing wrong-level registers causes undefined instruction exceptions

### Vector Table Alignment
- Exception vector table MUST be aligned to **2048 bytes (0x800)**
- Specified in **ISA Section D1.10.2**
- Linker script enforces this with `.align 11` (2^11 = 2048)

## Implementation Checklist

When implementing ARM-specific features:

- [ ] Cite ARM doc section number in code comments
- [ ] Document A72-specific behavior vs generic ARMv8-A
- [ ] Note QEMU vs hardware differences
- [ ] Include register bitfield diagrams for complex registers
- [ ] Cross-reference related system registers

## Related Documentation

- [Boot Sequence](../architecture/boot-sequence.md) - Exception level at boot
- [Exception Handling](../architecture/exceptions.md) - Vector table implementation
- [Memory Map](../hardware/memory-map.md) - GIC base address

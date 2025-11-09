# DaedalusOS - AI Assistant Guide

**Quick routing guide for AI agents working on DaedalusOS.**

## Project Essentials

- **Target**: Raspberry Pi 4 Model B only (BCM2711, Cortex-A72)
- **Language**: Rust 2024, nightly, `#![no_std]` bare-metal
- **Architecture**: AArch64 (ARMv8-A)
- **Current State**: Phase 1 complete - interactive shell with 25 passing tests
- **Next Milestone**: Heap allocator (Phase 2)

## Quick Commands

```bash
cargo build              # Build kernel ELF
cargo run                # Build + run in QEMU (interactive shell)
cargo test               # Run 25 tests in QEMU
./scripts/build-docs.sh  # Build unified docs (mdbook + cargo doc)
mdbook serve             # View docs at localhost:3000
```

**QEMU Requirement**: 9.0+ for raspi4b machine type (see `book/src/decisions/adr-002-qemu-9.md`)

## Documentation Map

All documentation is in **`book/src/`** organized as reference wiki (not linear).

### Hardware Specifications

**When**: Implementing drivers, debugging hardware issues
- **Memory map & addresses**: `book/src/hardware/memory-map.md`
- **UART (PL011)**: `book/src/hardware/uart-pl011.md` (includes baud rate calc, registers)
- **GPIO**: `book/src/hardware/gpio.md` (stub - not yet implemented)
- **Timer**: `book/src/hardware/timer.md` (stub)
- **GIC interrupts**: `book/src/hardware/gic.md` (stub - Phase 3)

### Architecture & Boot

**When**: Understanding boot flow, exceptions, memory layout
- **Boot sequence**: `book/src/architecture/boot-sequence.md` (firmware → ASM → Rust)
- **Exception handling**: `book/src/architecture/exceptions.md` (vectors, ESR/FAR, context save)
- **Linker script**: `book/src/architecture/linker-script.md` (section placement, symbols)
- **MMU/Paging**: `book/src/architecture/mmu-paging.md` (stub - Phase 2/3)

### External References

**When**: Need ARM docs, Pi datasheets, or learning resources
- **ARM documentation**: `book/src/references/arm.md` (ISA, Cortex-A72 TRM, GIC)
- **Raspberry Pi docs**: `book/src/references/raspberry-pi.md` (BCM2711, schematics, config.txt)
- **Similar projects**: `book/src/references/similar-projects.md` (Blog OS, Rust Pi OS, OSDev)

### Design Decisions

**When**: Understanding "why" behind architectural choices
- **Why Pi 4 only**: `book/src/decisions/adr-001-pi-only.md`
- **Why QEMU 9.0+**: `book/src/decisions/adr-002-qemu-9.md`

### Project Planning

- **Roadmap**: `book/src/roadmap.md` (phases, milestones, timeline)
- **Introduction**: `book/src/intro.md` (overview, current status)

## Critical Constants (Memorize These)

| Constant | Value | Source |
|----------|-------|--------|
| **Kernel load address** | `0x00080000` | Firmware entry point |
| **MMIO base (ARM)** | `0xFE000000` | BCM2711 ARM mapping (NOT 0x3F000000!) |
| **UART base** | `0xFE201000` | PL011 registers |
| **UART clock** | 54 MHz | Pi 4 specific (Pi 3 = 48 MHz) |
| **GIC distributor** | `0xFF841000` | Interrupt controller |
| **System timer** | `0xFE003000` | Timing functions |

## Context Optimization Strategy

### For Specific Queries

| Query Type | Read This | Don't Read |
|------------|-----------|------------|
| "UART init sequence" | `hardware/uart-pl011.md` | Other hardware docs |
| "Boot flow" | `architecture/boot-sequence.md` | Exception/linker docs |
| "Exception handling" | `architecture/exceptions.md` | Boot/UART docs |
| "Memory addresses" | `hardware/memory-map.md` | Implementation details |
| "ARM register details" | `references/arm.md` → specific section | Entire TRM |

### Progressive Disclosure

1. **Start here**: Relevant `.md` file (80 lines avg)
2. **Need more**: External reference links in "References" section
3. **Deep dive**: Full ARM TRM/BCM2711 PDF (cite specific sections in code)

**Efficiency**: Read 80-200 lines (targeted doc) vs 400+ lines (PROJECT.md)

## Unsafe Code Requirements

**CRITICAL**: Every `unsafe` block MUST have `// SAFETY:` comment explaining:
1. Which invariants are relied upon
2. Pre-conditions checked before the block
3. Type guarantees that ensure safety

**Reference**: `book/src/architecture/boot-sequence.md` for examples

## Code Organization

```
src/
├── main.rs              # Binary entry, panic handlers
├── lib.rs               # Print macros, test framework, init
├── shell.rs             # Interactive REPL
├── exceptions.rs        # Exception handlers, ESR/FAR decoding
├── drivers/
│   └── uart.rs          # PL011 driver (TX/RX)
├── qemu.rs              # Semihosting utilities
└── arch/aarch64/
    ├── boot.s           # Assembly entry, core parking
    └── exceptions.s     # Exception vector table
```

## Development Workflow

1. **Read relevant doc** from `book/src/`
2. **Implement feature** with hardware reference comments
3. **Update docs** if behavior changes
4. **Run tests**: `cargo test` (all must pass)
5. **Verify in QEMU**: `cargo run`

## AI Agent Best Practices

**DO:**
- ✅ Read targeted doc file (e.g., `uart-pl011.md`) for specific info
- ✅ Cross-reference ARM/Pi docs via links in documentation
- ✅ Cite specific ARM TRM sections in code comments
- ✅ Update relevant `.md` when implementation changes
- ✅ Verify constants match across code and docs

**DON'T:**
- ❌ Read entire PROJECT.md (400+ lines) for specific query
- ❌ Assume Pi 3 addresses work on Pi 4
- ❌ Add `unsafe` without `// SAFETY:` comment
- ❌ Hardcode magic numbers without datasheet reference
- ❌ Skip testing after changes

## Common Pitfalls

1. **MMIO Base**: Pi 4 uses `0xFE000000`, NOT `0x3F000000` (Pi 3)
2. **UART Clock**: 54 MHz on Pi 4, NOT 48 MHz (affects baud rate)
3. **Exception Level**: QEMU boots EL2, hardware boots EL1 (affects register access)
4. **QEMU Version**: Must be 9.0+ for raspi4b machine type
5. **Stack Alignment**: Must be 16-byte aligned (ARM AAPCS)

## Getting Help

- **Unclear requirement**: Ask user for clarification
- **Missing hardware detail**: Check `references/arm.md` or `references/raspberry-pi.md`
- **Code doesn't match docs**: Verify against actual source (`src/`)
- **Build fails**: Check `.cargo/config.toml` and `rust-toolchain`

## Version Info

- **Documentation**: See `book/book/index.html` after `mdbook build`
- **API Reference**: Run `cargo doc --open` for Rust code docs
- **Source of Truth**: Code in `src/` + docs in `book/src/`

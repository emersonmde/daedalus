# DaedalusOS - AI Assistant Guide

**Quick routing guide for AI agents working on DaedalusOS.**

## Project Essentials

- **Target**: Raspberry Pi 4 Model B only (BCM2711, Cortex-A72)
- **Language**: Rust 2024, nightly, `#![no_std]` bare-metal
- **Architecture**: AArch64 (ARMv8-A)
- **Current State**: Phase 4 in progress - Ethernet driver foundation complete
- **Next Milestone**: Frame TX/RX (Milestone #13)
- **End Goal**: Network-enabled device for remote GPIO control via HTTP

## Dependency Philosophy

**ALWAYS prefer existing `no_std` crates over reimplementation:**

- `#![no_std]` means "no standard library", NOT "no dependencies"
- The Rust embedded ecosystem has excellent battle-tested crates
- Focus learning on hardware/driver layer, not protocol/algorithm reimplementation

**Examples of crates to use:**
- ✅ `smoltcp` - TCP/IP stack (no_std compatible)
- ✅ Allocator crates (linked_list_allocator, buddy allocators, etc.)
- ✅ `embedded-hal` - Hardware abstraction traits
- ✅ Data structure crates (heapless, etc.)

**Where to implement from scratch:**
- Hardware drivers (UART, GPIO, Ethernet PHY, etc.) - this is where the learning happens
- Platform-specific initialization (MMU, exceptions, boot)
- Integration/glue code between crates and hardware

**When in doubt:** Check if a `no_std` crate exists first!

### Crates to Consider (Future Reference)

**Currently Using:**
- `alloc` - Heap allocation primitives (Box, Vec, String, etc.)
- Standard Rust `core` library

**Planned for Networking (Phase 4):**
- `smoltcp` - TCP/IP stack with ARP, IPv4, UDP, TCP, ICMP, DNS
  - No-std, no-alloc capable (uses custom buffer management)
  - Battle-tested in embedded systems
  - Use for: Milestone #13-#15 (IP through Application protocols)

**Consider for Future Milestones:**
- `embedded-hal` - Hardware abstraction traits
  - Provides standard traits: GPIO, SPI, I2C, delays, PWM, ADC, etc.
  - Benefit: Access to ecosystem of sensor/peripheral drivers
  - Trade-off: Extra abstraction layer (Pi4-only project)
  - Use for: Milestone #23 (I2C/SPI) if using existing sensor drivers

- `linked_list_allocator` or `buddy_system_allocator` - Better allocators
  - Replaces current bump allocator
  - Adds free/reallocation support
  - Use for: Milestone #18 (Better Allocator)

- `heapless` - Static data structures (Vec, String, etc. with compile-time size)
  - Useful for interrupt handlers (no allocation)
  - Consider for: Network packet buffers, ring buffers

- `cortex-a` - ARMv8-A register access helpers
  - Provides type-safe wrappers for system registers
  - May simplify MMU/exception code
  - Trade-off: Currently have working raw implementations

- `bitflags` - Type-safe bit flag manipulation
  - Useful for: Hardware register fields
  - Consider for: GPIO, Ethernet, any complex register manipulation

- `spin` - Spinlock primitives
  - Use for: Milestone #19 (Multi-Core Support)
  - Provides Mutex, RwLock, Once for no-std

**Crates to Avoid:**
- Anything requiring `std` (obviously)
- Crates with HAL dependencies for other boards (STM32, nRF, etc.)
- Overly generic abstractions when direct hardware access is clearer

## Quick Commands

```bash
cargo build              # Build kernel ELF
cargo run                # Build + run in QEMU (interactive shell)
cargo test               # Run tests in QEMU
./scripts/build-docs.sh  # Build unified docs (mdbook + cargo doc)
mdbook serve             # View docs at localhost:3000
```

### Verify Everything is OK

Before committing or after making changes, run the pre-commit hook to verify everything passes:

```bash
./.githooks/pre-commit   # Run all checks: fmt, clippy, test, build
```

This runs:
- `cargo fmt --check` - Verify formatting (errors fail)
- `cargo clippy` - Check for lint issues (errors fail, warnings shown)
- `cargo doc` - Build documentation (errors fail, warnings shown)
- `cargo test` - Run all tests (failures fail)
- `cargo build --release` - Verify release build (errors fail, warnings shown)

**QEMU Requirement**: 9.0+ for raspi4b machine type (see `docs/src/decisions/adr-002-qemu-9.md`)

## Documentation Map

All documentation is in **`docs/src/`** organized as reference wiki (not linear).

### Hardware Specifications

**When**: Implementing drivers, debugging hardware issues
- **Memory map & addresses**: `docs/src/hardware/memory-map.md`
- **UART (PL011)**: `docs/src/hardware/uart-pl011.md` (includes baud rate calc, registers)
- **GPIO**: `docs/src/hardware/gpio.md` (BCM2711 GPIO driver)
- **Timer**: `docs/src/hardware/timer.md` (system timer with delays)
- **GIC interrupts**: `docs/src/hardware/gic.md` (GIC-400 interrupt controller)
- **Ethernet**: `docs/ethernet-driver-research.md` (GENET v5, BCM54213PE PHY)

### Architecture & Boot

**When**: Understanding boot flow, exceptions, memory layout
- **Boot sequence**: `docs/src/architecture/boot-sequence.md` (firmware → ASM → Rust)
- **Exception handling**: `docs/src/architecture/exceptions.md` (vectors, ESR/FAR, context save)
- **Linker script**: `docs/src/architecture/linker-script.md` (section placement, symbols)
- **Heap allocator**: `docs/src/architecture/allocator.md` (bump allocator, memory management)
- **MMU/Paging**: `docs/src/architecture/mmu-paging.md` (stub - Phase 2/3)

### External References

**When**: Need ARM docs, Pi datasheets, or learning resources
- **ARM documentation**: `docs/src/references/arm.md` (ISA, Cortex-A72 TRM, GIC)
- **Raspberry Pi docs**: `docs/src/references/raspberry-pi.md` (BCM2711, schematics, config.txt)
- **Similar projects**: `docs/src/references/similar-projects.md` (Blog OS, Rust Pi OS, OSDev)

### Design Decisions

**When**: Understanding "why" behind architectural choices
- **Why Pi 4 only**: `docs/src/decisions/adr-001-pi-only.md`
- **Why QEMU 9.0+**: `docs/src/decisions/adr-002-qemu-9.md`

### Project Planning

- **Roadmap**: `docs/src/roadmap.md` (phases, milestones, timeline)
- **Introduction**: `docs/src/intro.md` (overview, current status)

## Critical Constants (Memorize These)

| Constant | Value | Source |
|----------|-------|--------|
| **Kernel load address** | `0x00080000` | Firmware entry point |
| **MMIO base (ARM)** | `0xFE000000` | BCM2711 ARM mapping (NOT 0x3F000000!) |
| **UART base** | `0xFE201000` | PL011 registers |
| **UART clock** | 54 MHz | Pi 4 specific (Pi 3 = 48 MHz) |
| **GPIO base** | `0xFE200000` | GPIO controller |
| **GENET base** | `0xFD580000` | Ethernet MAC controller |
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

**Efficiency**: Read 80-200 lines (targeted doc) vs entire documentation tree

## Unsafe Code Requirements

**CRITICAL**: Every `unsafe` block MUST have `// SAFETY:` comment explaining:
1. Which invariants are relied upon
2. Pre-conditions checked before the block
3. Type guarantees that ensure safety

**Reference**: `docs/src/architecture/boot-sequence.md` for examples

## Code Organization

```
src/
├── main.rs              # Binary entry, panic handlers
├── lib.rs               # Print macros, test framework, init
├── shell.rs             # Interactive REPL
├── exceptions.rs        # Exception handlers, ESR/FAR decoding
├── drivers/
│   ├── uart.rs          # PL011 driver (TX/RX)
│   ├── gpio.rs          # BCM2711 GPIO driver
│   ├── genet.rs         # GENET v5 Ethernet MAC
│   ├── timer.rs         # System timer
│   └── gic.rs           # GIC-400 interrupt controller
├── net/
│   ├── ethernet.rs      # Ethernet frame handling
│   └── arp.rs           # ARP protocol
├── qemu.rs              # Semihosting utilities
└── arch/aarch64/
    ├── boot.s           # Assembly entry, core parking
    ├── exceptions.s     # Exception vector table
    └── mmu.rs           # MMU/paging configuration
```

## Development Workflow

**Standard workflow for implementing features:**

1. **Read relevant doc** from `docs/src/`
2. **Implement feature** with hardware reference comments
3. **Run `cargo fmt`** to fix formatting
4. **Run `./.githooks/pre-commit`** to verify all checks pass
5. **Fix any errors/warnings** shown by pre-commit
6. **Update documentation** after code verification passes
7. **Test interactively** in QEMU (user handles this)

### Pre-Commit Hook Details

The pre-commit hook (`./.githooks/pre-commit`) ensures code quality:

**What it runs:**
- `cargo fmt --check` - Formatting (fails on errors)
- `cargo clippy` - Linting (fails on errors, shows warnings)
- `cargo doc` - Documentation build (fails on errors, shows warnings)
- `cargo test` - All unit and integration tests (fails on test failures)
- `cargo build --release` - Release build verification (fails on errors, shows warnings)

**Common fixes:**
- **Formatting errors**: Run `cargo fmt` before pre-commit
- **Dead code warnings**: Add `#[allow(dead_code)]` to modules with future-use constants
- **Bare URL warnings**: Wrap URLs in angle brackets: `<https://...>`
- **Unused import warnings**: Remove or use the imports

**When to run:**
- ✅ After implementing features (before updating docs)
- ✅ Before considering a milestone complete
- ❌ Do not update documentation until pre-commit passes

## AI Agent Best Practices

**DO:**
- ✅ Read targeted doc file (e.g., `uart-pl011.md`) for specific info
- ✅ Cross-reference ARM/Pi docs via links in documentation
- ✅ Cite specific ARM TRM sections in code comments
- ✅ Update relevant `.md` when implementation changes
- ✅ Verify constants match across code and docs

**DON'T:**
- ❌ Read entire documentation tree for specific queries
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
- **Pre-commit fails**: Run `cargo fmt` first, then address specific errors shown

## Version Info

- **Documentation**: See `docs/book/index.html` after `mdbook build`
- **API Reference**: Run `cargo doc --open` for Rust code docs
- **Source of Truth**: Code in `src/` + docs in `docs/src/`

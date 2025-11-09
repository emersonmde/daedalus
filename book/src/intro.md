# DaedalusOS Documentation

DaedalusOS is a bare-metal Rust kernel for the Raspberry Pi 4 Model B, developed as a learning project to explore OS internals and low-level ARM hardware programming.

## Project Scope

- **Target Hardware**: Raspberry Pi 4 Model B only (BCM2711, Cortex-A72)
- **Language**: Rust 2024 edition, nightly toolchain
- **Architecture**: AArch64 (ARMv8-A)
- **Environment**: `#![no_std]`, bare-metal (no operating system)

## Current Status

**Milestone #7 Complete**: Interactive shell with exception handling
- Working REPL with command parsing and line editing
- 25 tests passing in QEMU
- Exception vector table with full register dumps
- UART driver with TX/RX support

**Next Milestone**: Heap allocator (Phase 2)

## Documentation Structure

This documentation is organized as a reference wiki, not a linear tutorial. Jump to any topic:

### For Implementation Work
- **Hardware specs**: See [Hardware Reference](#hardware-reference)
- **Boot process**: See [Boot Sequence](architecture/boot-sequence.md)
- **Exception handling**: See [Exception Handling](architecture/exceptions.md)

### For Understanding Design
- **Why Pi 4 only?**: See [ADR-001](decisions/adr-001-pi-only.md)
- **QEMU requirements**: See [ADR-002](decisions/adr-002-qemu-9.md)
- **Project roadmap**: See [Roadmap](roadmap.md)

### For Reference
- **ARM documentation**: See [ARM Documentation](references/arm.md)
- **Pi 4 datasheets**: See [Raspberry Pi Documentation](references/raspberry-pi.md)
- **Similar projects**: See [Similar Projects](references/similar-projects.md)

## Quick Commands

```bash
# Build kernel
cargo build

# Run in QEMU
cargo run

# Run tests
cargo test

# Generate kernel8.img for hardware
cargo build --release
cargo objcopy --release -- -O binary kernel8.img
```

## Design Tenets

1. **Pi-Only, Tutorial-Inspired** - Port patterns from Phil Opp's Blog OS when useful
2. **Document One-Way Doors** - Major architecture decisions require ADRs
3. **Hardware Facts Over Assumptions** - Every magic number must reference datasheets
4. **Keep Build/Test Simple** - One target spec, one QEMU command
5. **Tight Feedback Loop** - Every milestone must build and run in QEMU

See [Design Decisions](#design-decisions) for detailed rationale.

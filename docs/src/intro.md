# DaedalusOS Documentation

DaedalusOS is a bare-metal Rust kernel for the Raspberry Pi 4 Model B, developed as a learning project to explore OS internals and low-level ARM hardware programming.

## Project Scope

- **Target Hardware**: Raspberry Pi 4 Model B only (BCM2711, Cortex-A72)
- **Language**: Rust 2024 edition, nightly toolchain
- **Architecture**: AArch64 (ARMv8-A)
- **Environment**: `#![no_std]`, bare-metal (no operating system)

## Current Status

**Phase 4 In Progress**: Networking Stack
**Milestone #12 Complete**: Ethernet Driver Foundation (GENET v5 + PHY)
- Working REPL with command parsing and shell history
- Exception vector table with register dumps
- 8 MB heap with bump allocator
- Full `alloc` crate support (Box, Vec, String, collections)
- System timer driver with microsecond precision delays
- GIC-400 interrupt controller with interrupt-driven UART
- MMU with 39-bit virtual address space (identity mapped)
- Caching enabled for performance
- GPIO driver with BCM2711 pull-up/down support
- **NetworkDevice trait abstraction** for hardware portability
- Ethernet frame and ARP protocol implementation (30 unit tests)
- GENET v5 MAC driver with MDIO/PHY management
- Hardware diagnostics command (`eth-diag`)

**Next Milestone**: Frame transmission and reception (TX/RX paths)

## Documentation Structure

This documentation is organized as a reference wiki, not a linear tutorial. Jump to any topic:

### For Implementation Work
- **Hardware specs**: See [Hardware Reference](#hardware-reference)
- **Boot process**: See [Boot Sequence](architecture/boot-sequence.md)
- **Exception handling**: See [Exception Handling](architecture/exceptions.md)

### For Understanding Design
- **Why Pi 4 only?**: See [ADR-001](decisions/adr-001-pi-only.md)
- **QEMU requirements**: See [ADR-002](decisions/adr-002-qemu-9.md)
- **Network device abstraction**: See [ADR-003](decisions/adr-003-network-device-trait.md)
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

# Run tests with deterministic timing (slower, but more reproducible in CI)
QEMU_DETERMINISTIC=1 cargo test

# Generate kernel8.img for hardware
cargo build --release
cargo objcopy --release -- -O binary kernel8.img
```

### Timing Tests in CI

If timing tests become flaky in GitHub Actions or other CI environments, you can enable deterministic timing mode using `QEMU_DETERMINISTIC=1`. This uses QEMU's `-icount` flag to decouple the guest clock from the host, making timing perfectly reproducible at the cost of 10-100x slower execution (disables KVM hardware acceleration).

Current timing tests use 25% tolerance to handle normal CI variability without this flag. See `src/drivers/timer.rs:231` for details.

## Design Tenets

1. **Pi-Only, Tutorial-Inspired** - Port patterns from Phil Opp's Blog OS when useful
2. **Document One-Way Doors** - Major architecture decisions require ADRs
3. **Hardware Facts Over Assumptions** - Every magic number must reference datasheets
4. **Keep Build/Test Simple** - One target spec, one QEMU command
5. **Tight Feedback Loop** - Every milestone must build and run in QEMU

See [Design Decisions](#design-decisions) for detailed rationale.

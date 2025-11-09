# ADR-001: Raspberry Pi 4 Only

**Status**: Accepted
**Date**: 2025-11-08
**Decision**: DaedalusOS targets only Raspberry Pi 4 Model B (BCM2711, Cortex-A72). No x86, no other ARM boards.

## Context

Originally inspired by Philipp Oppermann's Blog OS (x86_64), the project faced a choice:
1. **Maintain multi-architecture support** - Keep x86_64 builds alongside Pi 4
2. **Focus on single platform** - Pi 4 only, adapt patterns as needed
3. **Switch to generic ARM** - Target multiple ARM platforms

Supporting multiple architectures introduces complexity:
- Different boot processes (BIOS/UEFI vs firmware)
- Different memory maps and MMIO access
- Different interrupt controllers (APIC vs GIC)
- Different assemblycode for each platform
- Testing burden across platforms

## Decision

**Focus exclusively on Raspberry Pi 4 Model B.**

This is a **one-way door** decision. The codebase will:
- Use Pi 4-specific memory addresses (`0xFE000000` MMIO base)
- Rely on Pi 4 peripherals (PL011 UART, GIC-400, BCM2711 features)
- Drop x86_64 target specification and code
- Optimize for single platform instead of abstraction layers

## Rationale

1. **Learning focus**: Deep understanding of one platform > superficial knowledge of many
2. **Hardware access**: Actual Pi 4 hardware available for testing
3. **Simplicity**: No abstraction layers needed for hardware access
4. **Documentation**: Can cite specific datasheet sections without caveats
5. **Iteration speed**: One build target, one test platform, faster feedback

### Why Pi 4 Specifically?

- **Modern ARM**: ARMv8-A (64-bit) with contemporary features
- **Available hardware**: Widely available, affordable (~$35-75)
- **Good documentation**: BCM2711 peripherals PDF, ARM Cortex-A72 TRM
- **QEMU support**: raspi4b machine type (QEMU 9.0+)
- **Ecosystem**: Active community, learning resources

## Consequences

### Positive

- **Simpler codebase**: No platform abstraction, direct hardware access
- **Better documentation**: Can reference exact register addresses
- **Faster development**: One platform to test and verify
- **Deeper learning**: Master one SoC instead of many abstractions

### Negative

- **Not portable**: Cannot run on x86, other ARM boards, or cloud VMs
- **Historical code lost**: x86 code lives only in git history, will rot
- **Limited audience**: Only useful to Pi 4 owners/learners

### Neutral

- **Code reuse**: Patterns (print macros, testing) still portable to other projects
- **Future expansion**: Could add Pi 5 later if justified (new ADR required)

## Reversal Plan

If multi-architecture support becomes necessary:

1. **Create ADR-00X** documenting new scope and rationale
2. **Design HAL** (Hardware Abstraction Layer) separating platform code
3. **Restructure codebase**:
   ```
   src/
   ├── platform/
   │   ├── rpi4/     # Pi 4 specific
   │   └── x86_64/   # New platform
   ├── drivers/      # Generic drivers
   └── kernel/       # Platform-independent code
   ```
4. **Test on both platforms** before merging
5. **Update all documentation** for multi-platform reality

**Cost estimate**: 2-4 weeks of refactoring, significant ongoing testing burden.

## Current State

- x86_64 code removed from main branch (2025-11-08)
- Linker script, boot assembly, and memory map are Pi 4-specific
- All documentation assumes Pi 4 hardware

## Related Decisions

- [ADR-002: QEMU 9.0+ Requirement](adr-002-qemu-9.md) - Needed for raspi4b emulation

## References

- [BCM2711 Datasheet](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf)
- [ARM Cortex-A72 TRM](https://developer.arm.com/documentation/100095/0003)
- [Memory Map](../hardware/memory-map.md)

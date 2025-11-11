# Summary

[Introduction](intro.md)

---

# Hardware Reference

- [Memory Map](hardware/memory-map.md)
- [UART PL011](hardware/uart-pl011.md)
- [GPIO](hardware/gpio.md)
- [System Timer](hardware/timer.md)
- [GIC-400 Interrupt Controller](hardware/gic.md)
- [GENET Ethernet Controller](hardware/genet.md)
  - [Constant Verification](hardware/genet-verification.md)

# Architecture

- [Boot Sequence](architecture/boot-sequence.md)
- [Exception Handling](architecture/exceptions.md)
- [Linker Script](architecture/linker-script.md)
- [Heap Allocator](architecture/allocator.md)
- [MMU & Paging](architecture/mmu-paging.md)
- [Testing Framework](architecture/testing.md)
- [Network Protocol Stack](architecture/networking.md)

# Guides

- [Networking Guide](networking-guide.md)

# External References

- [ARM Documentation](references/arm.md)
- [Raspberry Pi Documentation](references/raspberry-pi.md)
- [Similar Projects](references/similar-projects.md)

# Design Decisions

- [About ADRs](decisions/README.md)
- [ADR-001: Raspberry Pi 4 Only](decisions/adr-001-pi-only.md)
- [ADR-002: QEMU 9.0+ Requirement](decisions/adr-002-qemu-9.md)
- [ADR-003: Network Device Abstraction](decisions/adr-003-network-device-trait.md)

---

[Roadmap](roadmap.md)
[API Reference](rustdoc/index.html)

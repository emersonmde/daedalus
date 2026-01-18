# DaedalusOS - AI Assistant Guide

**Quick reference for AI agents working on DaedalusOS.**

## Project Essentials

**CRITICAL CONTEXT:**
- **Target**: Raspberry Pi 4 Model B (BCM2711, Cortex-A72, ARMv8-A)
- **Language**: Rust 2024 edition, nightly, `#![no_std]` bare-metal
- **Current Milestone**: #15 (ARP Responder) - Milestone #14 complete (Jan 10, 2026)
- **End Goal**: Network-enabled device for remote GPIO control via HTTP
- **Learning Focus**: Hardware/driver layer (implement), protocols (use `no_std` crates)

**Future Hardware**: Pi 5 support planned via runtime detection (see ADR-005)

## Critical Constants - Memorize These

| Constant | Value | Notes |
|----------|-------|-------|
| **MMIO base** | `0xFE000000` | NOT 0x3F000000 (Pi 3)! |
| **UART base** | `0xFE201000` | PL011 UART |
| **UART clock** | 54 MHz | NOT 48 MHz (Pi 3) |
| **GPIO base** | `0xFE200000` | BCM2711 GPIO |
| **GENET base** | `0xFD580000` | Ethernet MAC |
| **GIC distributor** | `0xFF841000` | Interrupt controller |
| **Mailbox base** | `0xFE00B880` | VideoCore mailbox |
| **VC bus offset** | `0xC0000000` | ARM phys → VC bus |
| **Kernel entry** | `0x00080000` | Firmware load address |

## Quick Commands

```bash
# Development
cargo run                # Build + QEMU interactive shell
cargo test               # Run tests in QEMU
cargo fmt                # Format code (run before commit)
./.githooks/pre-commit   # Verify: fmt, clippy, doc, test, build

# Documentation
./scripts/build-docs.sh  # Build unified docs
mdbook serve             # View at localhost:3000
cargo doc --open         # API reference

# Shell Commands (in QEMU)
help                     # List all commands
gpio-mode, gpio-set      # GPIO control
eth-stats, netstats      # Network statistics
arp-probe                # Full networking test
```

**QEMU Requirement**: 9.0+ for raspi4b machine (see ADR-002)

## Current Architecture Patterns

### 1. Interrupt-Safe Locking
```rust
use crate::sync::Mutex;  // NOT core::sync::Mutex

static FOO: Mutex<Bar> = Mutex::new(Bar::new());
// Disables IRQs while locked - safe for interrupt handlers
```

**When to use**: Sharing data between interrupt handlers and normal code.

### 2. Socket Buffers (sk_buff)
```rust
use crate::net::skbuff::SkBuff;

let skb = SkBuff::new(data)?;  // Arc-based reference counting
// Linux-inspired packet buffer design
```

**When to use**: Network packet handling with zero-copy semantics.

### 3. Protocol Handler Registry
```rust
use crate::net::protocol::{ProtocolHandler, register_handler};

register_handler(ETH_P_ARP, my_arp_handler);
// Extensible dispatch for packet types
```

**When to use**: Adding new network protocol handlers.

### 4. Device Tree (Runtime Detection)
```rust
use crate::dt::HardwareInfo;

let hw = HardwareInfo::from_firmware()?;
// Runtime hardware detection for multi-board support (ADR-005)
```

**When to use**: Hardware-specific initialization (future Pi 5 support).

### 5. NetworkDevice Trait
```rust
impl NetworkDevice for GenetController {
    fn is_present(&self) -> bool { /* check hardware */ }
    // Enables runtime driver selection
}
```

**When to use**: All new network drivers (see ADR-005 for full pattern).

## Code Organization

```
src/
├── main.rs, lib.rs      # Entry, macros, init
├── shell.rs             # Interactive REPL
├── qemu.rs              # Semihosting
│
├── arch/aarch64/        # Architecture-specific
│   ├── boot.s           # ASM entry, core parking
│   ├── exceptions.{s,rs} # Vector table, handlers
│   └── mmu.rs           # MMU/paging
│
├── mm/allocator.rs      # Heap (bump allocator)
├── sync/mutex.rs        # Interrupt-safe Mutex
├── dt/mod.rs            # Device tree parsing
│
├── drivers/
│   ├── tty/serial/amba_pl011.rs
│   ├── gpio/bcm2711.rs
│   ├── mailbox/         # VideoCore communication
│   ├── net/
│   │   ├── netdev.rs    # NetworkDevice trait
│   │   └── ethernet/broadcom/genet.rs
│   ├── irqchip/gic_v2.rs
│   └── clocksource/bcm2711.rs
│
└── net/                 # Network stack
    ├── ethernet.rs      # Frame handling
    ├── arp.rs           # ARP protocol (legacy)
    ├── skbuff.rs        # Socket buffers
    ├── protocol.rs      # Handler registry
    ├── protocols/       # Protocol handlers
    │   └── arp.rs       # ARP handler
    └── socket/          # AF_PACKET sockets
        ├── mod.rs, types.rs
        ├── queue.rs     # Lock-free RX queues
        └── table.rs     # Socket table
```

## Dependencies Strategy

**IMPORTANT**: Prefer `no_std` crates over reimplementation!

**Currently Using:**
- `fdt-rs` - Device tree parsing (hardware detection)
- `lazy_static` + `spin_no_std` - Static initialization
- `spin` - Spinlock primitives
- `volatile` - Volatile memory access
- Core `alloc` crate (Box, Vec, String, etc.)

**Planned Next:**
- `smoltcp` - Full TCP/IP stack (Milestone #16)

**For Future Milestones:**
- `linked_list_allocator` - Better allocator (#18)
- `embedded-hal` - HAL traits if using sensor drivers (#24)
- `heapless` - Static data structures for interrupts

**Avoid**: Anything requiring `std`, board-specific HALs (STM32, nRF)

## Documentation Quick Reference

**Hardware Specs** (`docs/src/hardware/`):
- `memory-map.md` - All peripheral addresses
- `uart-pl011.md`, `gpio.md`, `timer.md`, `gic.md`
- `genet.md` - Ethernet controller + PHY
- `genet-verification.md` - Constant verification sources

**Architecture** (`docs/src/architecture/`):
- `boot-sequence.md` - Firmware → ASM → Rust
- `exceptions.md` - Vector table, ESR/FAR
- `allocator.md`, `mmu-paging.md`

**Decisions** (`docs/src/decisions/`):
- ADR-001: Why Pi 4 only
- ADR-002: Why QEMU 9.0+
- ADR-004: Filesystem structure
- ADR-005: **Multi-board support strategy** (runtime detection)

**References**: `docs/src/references/` - ARM docs, Pi datasheets, similar projects

## Development Workflow

**IMPORTANT - Follow this order:**

1. **Read relevant docs** from `docs/src/`
2. **Implement feature** with hardware reference comments
3. **Run `cargo fmt`** (fixes formatting)
4. **Run `./.githooks/pre-commit`** (verifies everything)
5. **Fix errors/warnings** shown by pre-commit
6. **Update documentation** (only after pre-commit passes)
7. User tests interactively in QEMU

### Pre-Commit Hook

Runs on every commit (or manually):
- `cargo fmt --check` - Formatting
- `cargo clippy` - Linting
- `cargo doc` - Doc build
- `cargo test` - All tests
- `cargo build --release` - Release build

**Common fixes:**
- Formatting: Run `cargo fmt`
- Dead code: Add `#[allow(dead_code)]` for future-use constants
- Bare URLs: Wrap in angle brackets `<https://...>`
- Unused imports: Remove them

## Unsafe Code Requirements

**CRITICAL**: Every `unsafe` block needs `// SAFETY:` comment with:
1. Which invariants are relied upon
2. Pre-conditions checked before the block
3. Type guarantees ensuring safety

See `docs/src/architecture/boot-sequence.md` for examples.

## Architecture Decision Records (ADRs)

**When to write**: Ask "Would future-me wonder why this exists?"

Create ADR when:
- One-way door decisions (hard to reverse)
- Non-obvious trade-offs
- Future-facing design (complexity now for later benefit)
- Breaking conventions

**Template**: See `docs/src/decisions/README.md`
**Format**: adr-NNN-short-title.md (zero-padded sequential)

## Common Pitfalls

1. **MMIO Base**: Pi 4 = `0xFE000000`, Pi 3 = `0x3F000000` - DON'T mix!
2. **UART Clock**: Pi 4 = 54 MHz, Pi 3 = 48 MHz (affects baud rate)
3. **VideoCore Bus**: Add `0xC0000000` to ARM physical addresses
4. **Mailbox Alignment**: 64-byte aligned (cache line), not 16-byte
5. **Exception Level**: Kernel runs at EL1 (boot.s drops from EL2)
6. **Stack Alignment**: 16-byte aligned (ARM AAPCS requirement)

## AI Agent Best Practices

**DO:**
- ✅ Read targeted doc files (e.g., `uart-pl011.md`) for specific info
- ✅ Cite ARM TRM sections in code comments (e.g., "See ARM DDI 0487, D1.10.2")
- ✅ Verify constants against datasheets
- ✅ Update relevant `.md` when implementation changes
- ✅ Use Task tool for exploration (not direct Grep/Glob for open-ended queries)

**DON'T:**
- ❌ Assume Pi 3 addresses/clocks work on Pi 4
- ❌ Add `unsafe` without `// SAFETY:` comment
- ❌ Hardcode magic numbers without datasheet references
- ❌ Update docs before pre-commit passes
- ❌ Read entire doc tree for specific queries

## Getting Help

- **Unclear requirement**: Ask user
- **Missing hardware detail**: Check `references/arm.md` or `references/raspberry-pi.md`
- **Code/docs mismatch**: Trust code in `src/`
- **Build fails**: Check `.cargo/config.toml`, `rust-toolchain`
- **Pre-commit fails**: Run `cargo fmt`, then fix specific errors

---

**Last Updated**: January 2026 (Milestone #14 complete)
**Source of Truth**: Code in `src/` + docs in `docs/src/`

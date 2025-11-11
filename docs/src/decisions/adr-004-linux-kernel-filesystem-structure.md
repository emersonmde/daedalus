# ADR-004: Linux Kernel Filesystem Structure

**Status**: Accepted
**Date**: 2025-11-11
**Decision**: Reorganize source tree following Linux kernel subsystem conventions where they improve maintainability, simplicity, and ease of implementation.

## Context

DaedalusOS currently uses a flat driver directory structure:

```
src/
├── drivers/
│   ├── uart.rs      # PL011 UART driver
│   ├── gpio.rs      # BCM2711 GPIO driver
│   ├── genet.rs     # Broadcom GENET ethernet
│   ├── gic.rs       # GIC-400 interrupt controller
│   ├── timer.rs     # BCM2711 system timer
│   └── netdev.rs    # NetworkDevice trait
├── net/             # Protocol stack
├── arch/aarch64/    # Architecture-specific code
├── allocator.rs     # Heap allocator
└── exceptions.rs    # Exception handling
```

### Problems with Current Structure

1. **Poor Scalability**: Flat `drivers/` directory will become cluttered as we add:
   - Multiple network drivers (WiFi, USB ethernet, mock devices)
   - Additional serial devices (mini UART, console abstraction)
   - More interrupt controllers (if porting to other boards)

2. **Unclear Organization**: Files like `netdev.rs` sit alongside hardware drivers
   - Is `netdev.rs` a driver or an abstraction?
   - Where would a second GPIO driver go?

3. **Generic Naming**: Files like `uart.rs`, `gpio.rs`, `timer.rs` don't indicate:
   - Which hardware they support (PL011? BCM2711? Generic?)
   - Platform specificity (Pi 4 only)

4. **Convention Mismatch**: Structure doesn't match established patterns:
   - Linux kernel uses subsystem directories (`drivers/tty/`, `drivers/irqchip/`)
   - Experienced developers expect familiar layout
   - AI agents trained on Linux kernel code struggle with flat structures

5. **Missing Separation**: Architecture-independent concerns mixed with drivers:
   - `exceptions.rs` is AArch64-specific but lives in `src/`
   - `allocator.rs` is generic memory management but sits at top level

### Why Use Linux-Inspired Structure?

Linux kernel structure provides a proven foundation that balances multiple goals:
- **Proven scalability**: Structure handles thousands of drivers across decades
- **Clear conventions**: Established patterns for subsystem organization
- **Developer familiarity**: Most OS developers recognize the layout as a bonus
- **Agent familiarity**: LLMs trained on Linux kernel code navigate similar structures naturally
- **Maintainability**: Clear boundaries between subsystems reduce cognitive load

We use Linux conventions **where they align with our goals** (maintainability, simplicity, ease of implementation), not as a strict requirement.

### Alternatives Considered

#### Alternative 1: Keep Current Flat Structure

**Pros**: Simple, no migration work
**Cons**: Doesn't scale, unclear organization, convention mismatch
**Rejected**: Already causing confusion about where new files should go

#### Alternative 2: Custom Hierarchical Structure

```
src/
├── devices/
│   ├── serial/uart.rs
│   ├── gpio/gpio.rs
│   └── network/genet.rs
├── memory/allocator.rs
└── interrupts/gic.rs
```

**Pros**: Cleaner than flat, custom to our needs
**Cons**: Unfamiliar to everyone, reinventing conventions
**Rejected**: No benefit over established Linux structure

#### Alternative 3: Minimal Rust-Idiomatic Structure

```
src/
├── hal/              # Hardware Abstraction Layer
│   ├── uart.rs
│   └── gpio.rs
├── drivers/          # High-level drivers
│   └── network.rs
└── platform/         # Platform-specific code
    └── bcm2711/
```

**Pros**: Matches embedded Rust `embedded-hal` pattern
**Cons**: Doesn't match OS development conventions, unclear boundaries
**Rejected**: DaedalusOS is an OS, not an embedded HAL library

## Decision

**Reorganize source tree following Linux kernel subsystem conventions** for improved maintainability and developer familiarity, using deep nesting and specific chip/device naming.

### Target Structure

```
src/
├── main.rs
├── lib.rs
├── shell.rs
├── qemu.rs
│
├── mm/                              # Memory Management (Linux: mm/)
│   ├── mod.rs
│   └── allocator.rs
│
├── arch/                            # Architecture-specific (Linux: arch/)
│   └── aarch64/
│       ├── mod.rs
│       ├── boot.s
│       ├── exceptions.s
│       ├── exceptions.rs            # ← Move from src/
│       └── mmu.rs
│
├── drivers/                         # Device Drivers (Linux: drivers/)
│   ├── mod.rs
│   │
│   ├── tty/                         # TTY subsystem (Linux: drivers/tty/)
│   │   ├── mod.rs
│   │   └── serial/
│   │       ├── mod.rs
│   │       └── amba_pl011.rs        # ← Rename uart.rs, match amba-pl011.c
│   │
│   ├── gpio/                        # GPIO subsystem (Linux: drivers/gpio/)
│   │   ├── mod.rs
│   │   └── bcm2711.rs               # ← Rename gpio.rs, chip-specific
│   │
│   ├── net/                         # Network devices (Linux: drivers/net/)
│   │   ├── mod.rs
│   │   ├── netdev.rs                # NetworkDevice trait
│   │   └── ethernet/
│   │       ├── mod.rs
│   │       └── broadcom/
│   │           ├── mod.rs
│   │           └── genet.rs         # ← Move from drivers/
│   │
│   ├── irqchip/                     # Interrupt controllers (Linux: drivers/irqchip/)
│   │   ├── mod.rs
│   │   └── gic_v2.rs                # ← Rename gic.rs, GIC-400 is v2
│   │
│   └── clocksource/                 # Timers (Linux: drivers/clocksource/)
│       ├── mod.rs
│       └── bcm2711.rs               # ← Rename timer.rs
│
└── net/                             # Network Protocol Stack (Linux: net/)
    ├── mod.rs
    ├── ethernet.rs
    └── arp.rs
```

### File Migrations

| Current Path | New Path | Linux Reference |
|--------------|----------|-----------------|
| `src/allocator.rs` | `src/mm/allocator.rs` | `mm/slab.c` |
| `src/exceptions.rs` | `src/arch/aarch64/exceptions.rs` | `arch/arm64/kernel/traps.c` |
| `src/drivers/uart.rs` | `src/drivers/tty/serial/amba_pl011.rs` | `drivers/tty/serial/amba-pl011.c` |
| `src/drivers/gpio.rs` | `src/drivers/gpio/bcm2711.rs` | `drivers/gpio/gpio-bcm2711.c` |
| `src/drivers/genet.rs` | `src/drivers/net/ethernet/broadcom/genet.rs` | `drivers/net/ethernet/broadcom/genet/bcmgenet.c` |
| `src/drivers/gic.rs` | `src/drivers/irqchip/gic_v2.rs` | `drivers/irqchip/irq-gic.c` |
| `src/drivers/timer.rs` | `src/drivers/clocksource/bcm2711.rs` | `drivers/clocksource/bcm2835_timer.c` |
| `src/drivers/netdev.rs` | `src/drivers/net/netdev.rs` | `include/linux/netdevice.h` |

### Naming Conventions

**Use specific, clear names** (happens to align with Linux patterns):
- Use specific chip/device names: `bcm2711.rs`, `amba_pl011.rs` (not ambiguous `gpio.rs`, `uart.rs`)
- Use underscores in Rust filenames: `gic_v2.rs` (Rust convention, adapted from Linux `irq-gic.c`)
- Use descriptive subsystem names: `irqchip/`, `clocksource/` (clarifies purpose better than `irq/`, `timer/`)

## Rationale

### Why Deep Nesting?

**Objection**: "Rust prefers flat modules, deep nesting is un-idiomatic"

**Primary benefit - Better organization**:
- Prevents cluttered flat directories as driver count grows
- Clear subsystem boundaries improve maintainability
- Vendor/chip-specific directories group related code naturally
- Easy to find where new drivers should go

**Technical compatibility - Rust handles nesting well**:
- Rust's module system handles deep nesting naturally via `mod.rs` files
- No impact on compilation, borrow checking, or lifetimes
- `pub use` re-exports provide clean public API when needed
- Cargo handles nested modules automatically

**Real-world Rust OS examples** also choose nested structures:
- [Redox OS](https://gitlab.redox-os.org/redox-os/kernel): Uses nested driver structure
- [Theseus OS](https://github.com/theseus-os/Theseus): Uses subsystem directories
- [Blog OS](https://github.com/phil-opp/blog_os): Small project, but uses `arch/` separation

### Why Specific Naming (bcm2711.rs vs gpio.rs)?

**Primary benefit - Eliminates ambiguity**:

Generic names create confusion as the codebase grows:
- `gpio.rs` - Which GPIO controller? BCM2711? RP2040? Abstract trait?
- `uart.rs` - PL011? Mini UART? 16550? Multiple implementations?
- `timer.rs` - System timer? ARM generic timer? Watchdog timer?

Specific chip/device names provide immediate clarity:
- `bcm2711.rs` - Unmistakably the BCM2711 GPIO driver
- `amba_pl011.rs` - Clearly ARM's PL011 UART (portable to other SoCs using PL011)
- `genet.rs` under `broadcom/` - Broadcom's GENET MAC, not Intel or Realtek

**Secondary benefit - Enables multiple implementations naturally**:
```
drivers/gpio/
├── mod.rs
├── bcm2711.rs        # Pi 4 GPIO
└── bcm2835.rs        # Pi 1-3 GPIO (if we add legacy support)
```

This also happens to match Linux naming conventions (`gpio-bcm2711.c`, `amba-pl011.c`), providing familiar patterns as a bonus.

### Why Follow Linux Conventions (Not Exact Matching)?

We adopt Linux naming and organization **where it improves maintainability**, not for strict conformance:

**Clear organization principles**: Linux conventions solve real problems:
- Subsystem boundaries (`drivers/tty/` vs `drivers/net/`) prevent mixing concerns
- Vendor directories (`ethernet/broadcom/`) naturally scale with multiple vendors
- Function-based naming (`irqchip/`, `clocksource/`) clarifies purpose

**Reduced cognitive load**: Familiar patterns require less mental mapping:
- "Where do serial drivers go?" → `drivers/tty/serial/` is the obvious answer
- New contributors don't waste time debating structure
- Clear precedent for where new code belongs

**Better tooling support**: AI agents and experienced developers benefit:
- LLMs trained on Linux kernel suggest correct file locations
- Agents understand context from directory structure without explanation
- Documentation references Linux subsystems for comparison

**We will deviate from Linux conventions when**:
- DaedalusOS-specific needs require different structure
- Rust idioms suggest clearer alternatives
- Simpler solutions exist for our single-platform scope

### Why Not embedded-hal Structure?

`embedded-hal` is a **library** for hardware abstraction, not an **operating system**.

**embedded-hal structure**:
```
src/
├── hal/              # Abstract traits
│   ├── gpio.rs
│   └── serial.rs
└── platform/         # Platform implementations
    └── bcm2711/
```

**Why this doesn't fit**:
- DaedalusOS is building an OS kernel, not a HAL library
- We need protocol stacks (`net/`), memory management (`mm/`), architecture code (`arch/`)
- Linux structure proven for OS development over 30+ years

## Consequences

### Positive

- **Scalability**: Clear place for new drivers (second network driver goes in `drivers/net/ethernet/vendor/`)
- **Familiarity**: Experienced OS developers immediately understand structure
- **AI effectiveness**: Agents trained on Linux kernel navigate codebase naturally
- **Clear boundaries**: Subsystems have obvious separation (`mm/`, `arch/`, `drivers/`)
- **Specific naming**: No ambiguity about which hardware a driver supports
- **Industry alignment**: Matches conventions of Linux, FreeBSD, Zircon

### Negative

- **Migration work**: ~50 files touched (imports updated)
- **Deeper paths**: `use crate::drivers::tty::serial::amba_pl011` vs `use crate::drivers::uart`
- **More directories**: 10+ new directories vs current 3
- **Breaking change**: External users (if any) must update imports

### Neutral

- **Compilation unchanged**: Rust module system handles nesting transparently
- **Performance unchanged**: File organization is compile-time only
- **Functionality unchanged**: Pure refactoring, no behavior changes

### Migration Impact

**Files to move**: 8 (allocator, exceptions, 6 drivers)
**New directories**: 10 (`mm/`, `drivers/tty/serial/`, `drivers/gpio/`, etc.)
**Import updates**: ~30-40 `use` statements across files
**Documentation updates**: CLAUDE.md, code reference sections
**Estimated time**: 1-2 hours (mostly mechanical)

## Implementation Plan

### Phase 1: Create Directory Structure

```bash
mkdir -p src/mm
mkdir -p src/drivers/{tty/serial,gpio,net/ethernet/broadcom,irqchip,clocksource}
```

### Phase 2: Move Files with Git (Preserve History)

```bash
# Memory management
git mv src/allocator.rs src/mm/allocator.rs

# Architecture
git mv src/exceptions.rs src/arch/aarch64/exceptions.rs

# Drivers
git mv src/drivers/uart.rs src/drivers/tty/serial/amba_pl011.rs
git mv src/drivers/gpio.rs src/drivers/gpio/bcm2711.rs
git mv src/drivers/genet.rs src/drivers/net/ethernet/broadcom/genet.rs
git mv src/drivers/netdev.rs src/drivers/net/netdev.rs
git mv src/drivers/gic.rs src/drivers/irqchip/gic_v2.rs
git mv src/drivers/timer.rs src/drivers/clocksource/bcm2711.rs
```

### Phase 3: Create mod.rs Files

Each directory needs a `mod.rs` for module declarations:

**`src/mm/mod.rs`**:
```rust
//! Memory Management subsystem
//! Corresponds to Linux mm/

pub mod allocator;
pub use allocator::*;
```

**`src/drivers/tty/mod.rs`**:
```rust
//! TTY and serial device drivers
//! Corresponds to Linux drivers/tty/

pub mod serial;
```

**`src/drivers/tty/serial/mod.rs`**:
```rust
//! Serial (UART) device drivers
//! Corresponds to Linux drivers/tty/serial/

pub mod amba_pl011;
pub use amba_pl011::*;
```

**`src/drivers/mod.rs`** (with backward compatibility):
```rust
//! Device drivers subsystem
//! Organized following Linux kernel conventions

pub mod tty;
pub mod gpio;
pub mod net;
pub mod irqchip;
pub mod clocksource;

// Backward compatibility aliases (remove in future breaking change)
pub mod uart {
    //! Deprecated: Use drivers::tty::serial instead
    pub use crate::drivers::tty::serial::*;
}

pub mod gic {
    //! Deprecated: Use drivers::irqchip::gic_v2 instead
    pub use crate::drivers::irqchip::gic_v2::*;
}

pub mod timer {
    //! Deprecated: Use drivers::clocksource instead
    pub use crate::drivers::clocksource::*;
}
```

### Phase 4: Update Imports

**Automated with search/replace**:

```rust
// Old imports
use crate::drivers::uart;
use crate::drivers::gic;
use crate::allocator;
use crate::exceptions;

// New imports (backward compatible via aliases)
use crate::drivers::uart;  // Still works via alias
use crate::drivers::gic;   // Still works via alias
use crate::mm::allocator;
use crate::arch::aarch64::exceptions;
```

**Or use new paths explicitly**:
```rust
use crate::drivers::tty::serial::amba_pl011;
use crate::drivers::irqchip::gic_v2;
```

### Phase 5: Update Documentation

- `CLAUDE.md`: Update file paths in "Code Organization" section
- `docs/src/hardware/*.md`: Update code reference paths
- `docs/src/architecture/*.md`: Update module paths
- `README.md`: Update getting started examples (if any)

### Phase 6: Remove Backward Compatibility (Future)

In next breaking version (v0.2.0 or v1.0.0):
- Remove alias modules from `drivers/mod.rs`
- Force all code to use new paths
- Update CLAUDE.md to remove old path references

## Testing

**Verification after migration**:
```bash
./.githooks/pre-commit  # Must pass:
                        # - cargo fmt --check
                        # - cargo clippy
                        # - cargo doc
                        # - cargo test
                        # - cargo build --release
```

**No functional changes**: All 66 tests must pass identically.

## Backward Compatibility

**Public API impact**: Low
- DaedalusOS is not a published library (no external consumers)
- Breaking change acceptable for v0.x versions

**Alias strategy**: Keep old paths working during transition:
```rust
// Old code continues working
use crate::drivers::uart::WRITER;  // Via alias

// New code uses explicit paths
use crate::drivers::tty::serial::amba_pl011::WRITER;
```

**Deprecation timeline**:
- v0.2.0: Add aliases, warn about deprecation in docs
- v0.3.0: Remove aliases, require new paths
- v1.0.0: Final structure solidified

## Related Decisions

- [ADR-001: Pi 4 Only](adr-001-pi-only.md) - Single platform simplifies driver organization
- [ADR-003: Network Device Abstraction](adr-003-network-device-trait.md) - NetworkDevice trait location: `drivers/net/netdev.rs`

## References

### Linux Kernel Structure

- **drivers/**: <https://elixir.bootlin.com/linux/latest/source/drivers>
  - `drivers/tty/serial/` - Serial device drivers
  - `drivers/gpio/` - GPIO controllers
  - `drivers/net/ethernet/` - Ethernet drivers (with vendor subdirs)
  - `drivers/irqchip/` - Interrupt controllers
  - `drivers/clocksource/` - Timer/clock drivers
- **mm/**: <https://elixir.bootlin.com/linux/latest/source/mm> - Memory management
- **arch/arm64/**: <https://elixir.bootlin.com/linux/latest/source/arch/arm64> - AArch64-specific code

### Other OS Structures

- **FreeBSD**: <https://github.com/freebsd/freebsd-src/tree/main/sys> - Similar subsystem organization
- **Redox OS**: <https://gitlab.redox-os.org/redox-os/kernel/-/tree/master/src/scheme> - Rust OS with driver organization
- **Zircon**: <https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/zircon/kernel/> - Google's kernel structure

### Rust OS Examples

- **Redox kernel**: <https://gitlab.redox-os.org/redox-os/kernel/-/blob/master/src/> - Nested driver structure
- **Theseus OS**: <https://github.com/theseus-os/Theseus/tree/theseus_main/kernel> - Subsystem directories

### Naming Conventions

- **Linux driver naming**: <https://www.kernel.org/doc/html/latest/process/coding-style.html>
- **Rust module conventions**: <https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html>

## Current State

- ✅ **Status**: Accepted and implemented
- ✅ **Implementation**: Complete (all files moved, mod.rs created, backward compatibility added)
- ✅ **Testing**: All 66 tests passing, build successful

## Questions for Review

1. **Nesting depth**: Is `drivers/net/ethernet/broadcom/genet.rs` too deep?
   - Alternative: `drivers/net/genet.rs` (one level)
   - Recommendation: Keep deep for future vendor expansion

2. **Backward compatibility**: Keep aliases indefinitely or remove in v0.2.0?
   - Recommendation: Remove in v0.2.0 (clean break while still v0.x)

3. **Timer naming**: `bcm2711.rs` (Pi 4-specific) or `bcm2835.rs` (Linux naming)?
   - Linux uses `bcm2835_timer.c` for backward compat even on Pi 4
   - Recommendation: `bcm2711.rs` (accurate for our Pi 4-only scope per ADR-001)

4. **arch/aarch64/exceptions.rs**: Keep or create `arch/aarch64/kernel/` subdir?
   - Linux has `arch/arm64/kernel/traps.c`, `arch/arm64/kernel/entry.S`
   - Recommendation: Keep flat for now, add `kernel/` if more arch files appear

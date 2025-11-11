# ADR-005: Multi-Board Support Strategy

**Status**: Accepted
**Date**: 2025-01-11
**Decision**: Use hybrid runtime hardware detection pattern to support multiple Raspberry Pi boards (Pi 4, Pi 5) with a single kernel binary.

## Context

DaedalusOS currently targets **only Raspberry Pi 4** (BCM2711 SoC) as documented in [ADR-001](adr-001-pi-only.md). However, Raspberry Pi 5 introduces significant architectural changes that require planning now:

### Pi 5 Architectural Changes (BCM2712 + RP1)

Pi 5 moves to a **disaggregated architecture** with most I/O offloaded to a separate RP1 I/O controller chip (Raspberry Pi's first custom silicon), connected via PCIe Gen 2.0:

| Peripheral | Pi 4 (BCM2711) | Pi 5 (BCM2712 + RP1) |
|------------|----------------|----------------------|
| **Ethernet** | GENET v5 (native) | RP1-Ethernet (PCIe-attached) |
| **USB** | DWC2 (native) | RP1-USB (PCIe-attached) |
| **GPIO** | BCM2711 registers | RP1 registers (PCIe-attached) |
| **UART** | PL011 (native) | RP1-UART (PCIe-attached) |
| **I2C/SPI** | BCM2711 | RP1 |

**Impact**: Almost all drivers will need board-specific implementations. The question is: how do we support both boards without massive rewrites?

### Timing Considerations

- **Current**: Pi 4 only, QEMU 9.0+ supports raspi4b machine
- **Near Future**: QEMU will add BCM2712/RP1 emulation (likely 2025)
- **User Context**: Developer has both Pi 4 and Pi 5 hardware, wants to support both

### The Problem

How do we architect driver support to:
1. Continue Pi 4 development without impediment
2. Add Pi 5 support cleanly when QEMU support arrives
3. Avoid major refactoring/rewrites when transitioning
4. Potentially support both boards with a single binary (convenience for testing/deployment)

## Decision

**Use hybrid runtime hardware detection pattern** (inspired by Linux driver probing):

1. **Driver Pattern**: All drivers implement `is_present()` hardware detection
2. **Trait Abstraction**: Multi-implementation categories use traits (`NetworkDevice`, future `UsbHost`)
3. **Runtime Selection**: At boot, probe for hardware and instantiate correct driver
4. **Single Binary**: One kernel image auto-detects board and initializes appropriate drivers

**Current Phase**: Document pattern now, implement multi-board support later (when QEMU adds Pi 5)

**Implementation**: Drivers follow the pattern starting now, enabling seamless Pi 5 addition without refactoring existing code.

## Rationale

### Why Hybrid Approach?

**"Hybrid"** means:
- **Now**: Single target (Pi 4), pattern documented but not exercised for multi-board
- **Later**: Same pattern enables runtime detection with zero driver changes
- **Best of both worlds**: No premature complexity, future-proof architecture

Key advantages:
1. **Zero overhead now**: Pattern doesn't complicate Pi 4-only development
2. **Additive Pi 5 support**: Add new driver files, no refactoring of working code
3. **Single binary convenience**: One image for both boards (testing/deployment)
4. **Linux-like familiarity**: Driver probing pattern is well-understood
5. **Already partially implemented**: NetworkDevice trait + GENET's `is_present()` already follow this

### Alternatives Considered

#### Alternative 1: Compile-Time Board Selection

Use Cargo features to select target board at compile time:

```rust
#[cfg(feature = "pi4")]
use drivers::net::ethernet::broadcom::genet::GenetController as NetDevice;

#[cfg(feature = "pi5")]
use drivers::net::ethernet::broadcom::rp1_enet::Rp1Ethernet as NetDevice;
```

Build separate binaries:
```bash
cargo build --features pi4    # Pi 4 kernel
cargo build --features pi5    # Pi 5 kernel
```

**Pros**:
- Simple, zero runtime overhead
- Smaller binaries (only one set of drivers compiled in)
- Clear separation of concerns

**Cons**:
- Need separate kernel images for each board
- Can't auto-detect board at boot (user must know which image to use)
- More build/release complexity (maintain two images)
- Testing requires rebuilding between boards

**Why rejected**: Inconvenient for users with multiple boards, requires manual image selection. Runtime overhead is negligible for bare-metal (no resource constraints).

#### Alternative 2: Pure Runtime Detection with Dynamic Dispatch

Always use trait objects with runtime dispatch:

```rust
// All drivers behind trait objects
static NETWORK: Mutex<Option<Box<dyn NetworkDevice>>> = Mutex::new(None);

fn init() {
    // Always probe all drivers
    if let Some(genet) = try_init_genet() {
        NETWORK.lock().replace(genet);
    } else if let Some(rp1) = try_init_rp1() {
        NETWORK.lock().replace(rp1);
    }
}
```

**Pros**:
- Maximum flexibility
- Clean abstraction boundaries
- Easy to add new boards

**Cons**:
- Overhead of dynamic dispatch (negligible in practice)
- All driver code compiled in (larger binary)
- More complex initialization infrastructure

**Why rejected**: Over-engineered for current needs. Hybrid approach gives same flexibility with simpler implementation.

#### Alternative 3: Device Tree-Driven (Linux Kernel Style)

Parse device tree at boot to discover hardware:

```rust
// Read device tree to find compatible devices
for node in devicetree.nodes() {
    if node.compatible("broadcom,genet-v5") {
        register_driver(GenetDriver);
    } else if node.compatible("raspberrypi,rp1-eth") {
        register_driver(Rp1Driver);
    }
}
```

**Pros**:
- Very flexible, supports unknown future boards
- Standard approach (used by Linux)
- External configuration (no recompile for new boards)

**Cons**:
- Need device tree parser (complex, ~5000+ lines in Linux)
- Need driver registration infrastructure
- Overkill for 2-board support
- Firmware must provide correct device tree

**Why rejected**: Too much infrastructure for minimal benefit. We control both supported boards, don't need external configuration.

## Consequences

### Positive

- **Future-proof**: Pi 5 support is additive (new files), not refactoring
- **Single binary option**: One kernel for both boards (convenience)
- **Existing pattern**: NetworkDevice trait already implements this approach
- **Clear guidelines**: Documented pattern prevents inconsistent implementations
- **Testable**: Can test Pi 4/Pi 5 code paths in same binary (future)
- **Familiar**: Linux-like driver probing pattern

### Negative

- **Larger binary**: Both driver sets compiled in (vs compile-time selection)
  - *Mitigation*: Bare-metal has no resource constraints, Pi 4 has 1-8 GB RAM
- **Runtime probe overhead**: Checking hardware at boot (~milliseconds)
  - *Mitigation*: One-time cost, negligible compared to boot time
- **Pattern requirements**: All drivers must follow pattern (documented in CLAUDE.md)
  - *Mitigation*: Pattern is simple (`is_present()` + trait), already partially implemented

### Neutral

- **No immediate changes**: Pattern documented, not yet exercised for multi-board
- **Deferred implementation**: Multi-board support waits for QEMU Pi 5 support
- **Some drivers don't need traits**: Single-implementation categories (timers, GPIO) use chip-specific naming instead

## Implementation Requirements

### Driver Pattern (Documented in CLAUDE.md)

All drivers must implement:

1. **Hardware Detection**:
```rust
impl MyDriver {
    pub fn is_present(&self) -> bool {
        // Read version/ID register to detect hardware
        let version = self.read_reg(VERSION_REG);
        version == EXPECTED_VERSION
    }
}
```

2. **Trait-Based Interfaces** (for multi-implementation categories):
```rust
pub trait NetworkDevice {
    fn is_present(&self) -> bool;
    fn init(&mut self) -> Result<(), Error>;
    // ...
}
```

3. **Self-Contained Initialization**:
```rust
impl MyDriver {
    pub fn new() -> Self { /* ... */ }
    pub fn init(&mut self) -> Result<(), Error> {
        if !self.is_present() {
            return Err(Error::HardwareNotPresent);
        }
        // Initialize hardware
        Ok(())
    }
}
```

### Directory Structure Rules

**Use deep structure** for categories with cross-vendor diversity:
```
drivers/net/ethernet/broadcom/  # Multiple ethernet vendors
drivers/net/wireless/            # Multiple WiFi vendors
drivers/usb/host/                # Multiple USB controllers
```

**Use flat structure** for single-vendor version changes:
```
drivers/gpio/bcm2711.rs         # Pi 4
drivers/gpio/rp1.rs             # Pi 5
```

### Future Runtime Detection (When Pi 5 Support Added)

```rust
// Detect network device at boot
let mut network_device: Box<dyn NetworkDevice> = {
    let genet = GenetController::new();
    if genet.is_present() {
        Box::new(genet)  // Pi 4
    } else {
        let rp1 = Rp1Ethernet::new();
        if rp1.is_present() {
            Box::new(rp1)  // Pi 5
        } else {
            panic!("No network hardware detected")
        }
    }
};

network_device.init()?;
```

## Current State

- ✅ **Pattern documented**: CLAUDE.md contains driver implementation guidelines
- ✅ **Partially implemented**: NetworkDevice trait + GENET `is_present()` already follow pattern
- ⏳ **Pi 5 implementation**: Waiting for QEMU BCM2712/RP1 emulation support
- ⏳ **Multi-board runtime detection**: Not yet implemented (only Pi 4 currently supported)

## Related Decisions

- [ADR-001: Pi 4 Only](adr-001-pi-only.md) - Current single-platform constraint
- [ADR-003: Network Device Abstraction](adr-003-network-device-trait.md) - NetworkDevice trait follows this pattern
- [ADR-004: Linux Kernel Filesystem Structure](adr-004-linux-kernel-filesystem-structure.md) - Directory structure supports vendor separation

## References

### Raspberry Pi 5 Architecture

- **RP1 Documentation**: [PiCockpit RP1 Overview](https://picockpit.com/raspberry-pi/i-read-the-rp1-documentation-so-you-dont-have-to/)
- **Pi 5 Announcement**: [Raspberry Pi Blog](https://www.raspberrypi.com/news/introducing-raspberry-pi-5/)
- **Architecture Analysis**: [EE News Europe - Disaggregated Architecture](https://www.eenewseurope.com/en/raspberry-pi-5-moves-to-disaggregated-architecture-with-in-house-silicon/)

### Driver Patterns

- **Linux Driver Model**: <https://www.kernel.org/doc/html/latest/driver-api/driver-model/>
- **Linux Device Probing**: <https://www.kernel.org/doc/html/latest/driver-api/device_link.html>
- **Rust Embedded Patterns**: <https://docs.rust-embedded.org/book/>

### Implementation

- **Pattern Documentation**: `CLAUDE.md` - "Multi-Board Support Strategy" section
- **Current Implementation**: `src/drivers/net/netdev.rs` - NetworkDevice trait
- **Example**: `src/drivers/net/ethernet/broadcom/genet.rs` - GENET driver with `is_present()`

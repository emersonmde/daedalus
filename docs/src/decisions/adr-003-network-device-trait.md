# ADR-003: Network Device Abstraction Layer

**Status**: Accepted
**Date**: 2025-11-10
**Decision**: Implement NetworkDevice trait abstraction for network hardware drivers.

## Context

DaedalusOS currently targets Raspberry Pi 4 exclusively ([ADR-001](adr-001-pi-only.md)), which uses the BCM2711 GENET v5 Ethernet controller. However, future expansion plans include:

1. **Raspberry Pi 5 support** - Different Ethernet controller (when QEMU support available)
2. **QEMU mock driver** - Enable network stack testing in emulation (Milestone #14)
3. **smoltcp integration** - TCP/IP stack expects generic device abstraction (Milestone #16)

Two architectural approaches were considered:

### Option A: Direct GENET Usage (No Abstraction)
```rust
// All network code directly uses GenetController
let mut genet = GenetController::new();
genet.init()?;
genet.transmit(frame)?;
```

**Pros**: Simpler initially, no abstraction overhead
**Cons**: Tight coupling, difficult to add Pi 5 or mock drivers later

### Option B: Trait Abstraction Now
```rust
// Network code uses trait, implementation is pluggable
let mut netdev: Box<dyn NetworkDevice> = Box::new(GenetController::new());
netdev.init()?;
netdev.transmit(frame)?;
```

**Pros**: Future-proof, testable, aligns with smoltcp patterns
**Cons**: Extra abstraction layer, more upfront design

### Option C: Minimal Trait Now, Full Implementation Later (Chosen)
```rust
// Trait exists, but only one implementation initially
trait NetworkDevice {
    fn init(&mut self) -> Result<(), NetworkError>;
    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError>;
    fn receive(&mut self) -> Option<&[u8]>;
    // ... minimal interface
}

impl NetworkDevice for GenetController { /* ... */ }
```

**Pros**: Captures design now, enables gradual implementation
**Cons**: None significant

## Decision

**Implement NetworkDevice trait abstraction in Milestone #12** (alongside protocol structures).

The trait provides:
- Hardware detection (`is_present()`)
- Lifecycle management (`init()`)
- Frame I/O (`transmit()`, `receive()`)
- Metadata (`mac_address()`, `link_up()`)

Current implementations:
- `GenetController` (Pi 4 GENET v5)

Future implementations:
- Mock device for QEMU (Milestone #14)
- Pi 5 Ethernet controller (when hardware available)

## Rationale

### Why Now (Milestone #12) Instead of Later?

1. **Low cost**: Trait definition is small (~100 lines), mostly documentation
2. **Captures design intent**: Documents interface requirements while fresh
3. **Enables testing**: Mock driver can be added in Milestone #14 without refactoring
4. **Aligns with smoltcp**: Their `Device` trait expects similar abstraction

### Why This Interface?

**Blocking transmit, non-blocking receive**:
- Simplifies initial implementation (interrupts come in Milestone #14)
- Common pattern in embedded networking (Linux `ndo_start_xmit`, smoltcp)
- API remains stable when adding interrupt-driven I/O

**Single-frame API (no queues)**:
- Pushes buffer management to implementation (GENET has hardware rings)
- Keeps trait simple and focused
- Protocol stacks (smoltcp) poll in loops and manage their own buffers

**Optional link_up() with default**:
- Not all devices have PHY link detection (mock drivers)
- Default returns `false` (conservative)
- Real hardware can override

**Result-based error handling**:
- `NetworkError` enum covers all failure modes
- Explicit errors better than silent failures in bare-metal

## Consequences

### Positive

- **Future-proof**: Adding Pi 5 or mock drivers requires no refactoring
- **Testable**: Can swap real hardware for mock in tests
- **smoltcp integration**: Clean Device trait implementation (wrap our trait)
- **Clear interface**: Documents exactly what network hardware must provide

### Negative

- **Abstraction overhead**: Extra trait layer (negligible in practice)
- **Not strictly needed**: Could delay until Pi 5 support (but harder to retrofit)

### Neutral

- **Current code unchanged**: GENET driver gains trait implementation, no functional changes
- **API stability**: Trait signature designed to remain stable through interrupt-driven I/O

## Implementation Details

### Module Structure

```
src/
├── drivers/
│   ├── netdev.rs         # Trait definition, NetworkError (NEW)
│   └── genet.rs          # impl NetworkDevice for GenetController
└── net/
    ├── ethernet.rs       # Ethernet protocol (uses trait in future)
    └── arp.rs            # ARP protocol
```

### Trait Definition

```rust
pub trait NetworkDevice {
    fn is_present(&self) -> bool;
    fn init(&mut self) -> Result<(), NetworkError>;
    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError>;
    fn receive(&mut self) -> Option<&[u8]>;
    fn mac_address(&self) -> MacAddress;
    fn link_up(&self) -> bool { false }  // Default implementation
}
```

### Error Types

```rust
pub enum NetworkError {
    HardwareNotPresent,
    NotInitialized,
    TxBufferFull,
    FrameTooLarge,
    FrameTooSmall,
    HardwareError,
    Timeout,
    InvalidConfiguration,
}
```

### Frame Size Validation

Trait implementations enforce Ethernet frame size constraints:
- **Minimum**: 60 bytes (excludes 4-byte CRC)
- **Maximum**: 1514 bytes (excludes 4-byte CRC)

Source: IEEE 802.3 Ethernet standard

## Design Patterns

### Pattern 1: Linux net_device

The Linux kernel uses `struct net_device` with function pointers:

```c
struct net_device_ops {
    int (*ndo_init)(struct net_device *dev);
    int (*ndo_start_xmit)(struct sk_buff *skb, struct net_device *dev);
    // ...
};
```

Our trait is the Rust equivalent with compile-time polymorphism.

### Pattern 2: embedded-hal

Rust embedded ecosystem uses trait abstractions:

```rust
pub trait SpiDevice {
    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error>;
}
```

Our `NetworkDevice` follows this pattern for bare-metal Rust.

### Pattern 3: smoltcp Device

smoltcp expects a `Device` trait:

```rust
pub trait Device {
    fn receive(&mut self, timestamp: Instant) -> Option<(Self::RxToken, Self::TxToken)>;
}
```

We'll implement smoltcp's trait by wrapping our `NetworkDevice` trait in Milestone #16.

## Testing Impact

### Unit Tests

Added `test_network_device_trait()` validating:
- Frame size validation (too small, too large)
- Error handling (NotInitialized state)
- MAC address retrieval

### Integration Tests (Future)

```rust
#[test_case]
fn test_mock_network_device() {
    let mut mock = MockNetworkDevice::new();
    mock.init().unwrap();

    // Inject test frame
    mock.inject_rx_frame(&test_frame);
    assert!(mock.receive().is_some());

    // Capture TX frames
    mock.transmit(&outgoing_frame).unwrap();
    assert_eq!(mock.captured_tx_frames().len(), 1);
}
```

## Migration Path

### Current (Milestone #12)

```rust
// Direct usage of trait implementation
use daedalus::drivers::genet::GenetController;
use daedalus::drivers::netdev::NetworkDevice;

let mut genet = GenetController::new();
if genet.is_present() {
    genet.init()?;
    genet.transmit(&frame)?;
}
```

### Future (Milestone #14+)

```rust
// Runtime selection of implementation
use daedalus::drivers::netdev::NetworkDevice;

let mut netdev: Box<dyn NetworkDevice> = if in_qemu() {
    Box::new(MockNetworkDevice::new())
} else {
    Box::new(GenetController::new())
};

netdev.init()?;
// Same API for both implementations
```

### smoltcp Integration (Milestone #16)

```rust
// Wrap our trait in smoltcp's Device trait
struct DaedalusDevice<T: NetworkDevice> {
    inner: T,
    rx_buffer: [u8; 1518],
}

impl<T: NetworkDevice> smoltcp::phy::Device for DaedalusDevice<T> {
    fn receive(&mut self, _timestamp: Instant) -> Option<(RxToken, TxToken)> {
        // Map our receive() to smoltcp's token API
    }
}
```

## Alternatives Considered

### Alternative 1: Delay Until Pi 5 Support

**Rejected**: Retrofitting abstraction later requires:
1. Refactoring all network code
2. Changing function signatures throughout codebase
3. Risk of breaking working code

Cost of adding trait now is minimal, benefit is substantial.

### Alternative 2: Use embedded-hal Traits

**Rejected**: `embedded-hal` doesn't define network device traits (only SPI, I2C, GPIO, etc.). We'd need to design our own anyway.

### Alternative 3: Function Pointers (C-style)

```rust
struct NetworkDevice {
    init: fn(&mut Self) -> Result<(), NetworkError>,
    transmit: fn(&mut Self, &[u8]) -> Result<(), NetworkError>,
    // ...
}
```

**Rejected**: Rust traits provide better type safety, compile-time dispatch, and zero-cost abstraction.

## Reversal Plan

If the abstraction proves unnecessary (e.g., we never add Pi 5 or mock drivers):

**To remove trait abstraction**:
1. Change all `use NetworkDevice` to direct `GenetController` usage
2. Replace trait method calls with direct GENET method calls
3. Delete `src/drivers/netdev.rs` (~290 lines)
4. Update documentation to remove trait references
5. Mark ADR-003 as "Deprecated - Abstraction not needed"

**Cost estimate**: ~2 hours (straightforward refactoring, all usage is local)

**Triggers for reversal**:
- Milestone #14 skipped (no QEMU mock driver implemented)
- Milestone #16 uses smoltcp differently (doesn't need our trait)
- Pi 5 support deemed out of scope permanently
- Trait adds measurable performance overhead (unlikely but possible)

**Likelihood**: Low. The trait is minimal (~100 lines) and already implemented. More likely we'll add implementations than remove the abstraction.

## Current State

- ✅ `NetworkDevice` trait defined (`src/drivers/netdev.rs`)
- ✅ `GenetController` implements trait
- ✅ 66 unit tests passing (added 1 new test)
- ✅ Documentation complete
- ⏳ Milestone #13 will use trait for TX/RX implementation

## Future Work

### Milestone #14: Mock Network Device

```rust
pub struct MockNetworkDevice {
    rx_queue: Vec<Vec<u8>>,
    tx_captured: Vec<Vec<u8>>,
    mac: MacAddress,
}

impl NetworkDevice for MockNetworkDevice {
    // Enable network stack testing in QEMU
}
```

### Milestone #16: smoltcp Integration

```rust
impl<T: NetworkDevice> smoltcp::phy::Device for DaedalusDevice<T> {
    // Bridge our trait to smoltcp's expectations
}
```

### Pi 5 Support (Future)

```rust
pub struct Pi5EthernetController { /* ... */ }

impl NetworkDevice for Pi5EthernetController {
    // Same interface, different hardware
}
```

## Related Decisions

- [ADR-001: Pi 4 Only](adr-001-pi-only.md) - Why single platform (but plan for expansion)
- Future ADR: Pi 5 Support (when QEMU gains raspi5b machine type)

## References

### Design Patterns

- **Linux net_device**: <https://elixir.bootlin.com/linux/latest/source/include/linux/netdevice.h>
- **embedded-hal traits**: <https://github.com/rust-embedded/embedded-hal>
- **smoltcp Device**: <https://docs.rs/smoltcp/latest/smoltcp/phy/trait.Device.html>

### Standards

- **IEEE 802.3**: Ethernet frame format and size constraints
- **RFC 1122**: Requirements for Internet Hosts (network layer expectations)

### Implementation

- Module: `src/drivers/netdev.rs`
- Usage: `src/drivers/genet.rs` (NetworkDevice implementation)
- Tests: `src/drivers/genet.rs::tests::test_network_device_trait`

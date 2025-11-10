# Ethernet Driver Research & Planning

**Milestone #12: Ethernet Driver (BCM54213PE PHY)**
**Status**: Research Phase
**Date Started**: 2025-11-09

---

## Phase 1: Initial Plan (Based on Existing Knowledge)

### Hardware Architecture

```
┌─────────────────┐
│   DaedalusOS    │
│   (Software)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐      MDIO/MDC      ┌──────────────┐
│ GENET Controller│◄─────────────────►│ BCM54213PE   │
│   (MAC Layer)   │                    │  PHY Chip    │
└────────┬────────┘                    └──────┬───────┘
         │                                     │
         │ RX/TX Buffers                      │ Physical Link
         │                                     │
         └─────────────────────────────────────┘
                   Ethernet Frames
```

### Hardware Components

1. **GENET (Gigabit Ethernet) Controller**
   - BCM2711 integrated MAC (Media Access Control) layer
   - Handles frame transmission/reception
   - DMA engine (we'll use simple mode initially)
   - Register interface for configuration
   - Interrupt support for RX/TX completion

2. **BCM54213PE PHY (Physical Layer)**
   - External chip on Pi 4
   - Communicates with GENET via MDIO/MDC bus
   - Handles link detection, auto-negotiation
   - Registers for status and configuration
   - Supports 10/100/1000 Mbps

3. **MDIO/MDC Bus**
   - Management interface between MAC and PHY
   - MDIO = data, MDC = clock
   - Used to read/write PHY registers (clause 22 protocol)

### Implementation Modules

**Module 1: GENET MAC Driver** (`src/drivers/genet.rs`)
- Controller reset and initialization sequence
- MAC address configuration (from OTP or hardcoded)
- RX/TX buffer management (simple mode, no DMA descriptors initially)
- Interrupt setup and handling (integrate with GIC)
- Frame transmission/reception API

**Module 2: MDIO Bus Driver** (part of genet.rs or separate)
- MDIO clause 22 protocol implementation
- PHY register read/write with proper timing
- Wait for operation completion

**Module 3: PHY Driver** (part of genet.rs or `src/drivers/phy.rs`)
- BCM54213PE-specific initialization sequence
- Link status detection and monitoring
- Auto-negotiation configuration and status
- Speed/duplex detection

**Module 4: Ethernet Frame Handling** (`src/net/ethernet.rs`)
- Frame structure definition
- Frame parsing (dest MAC, src MAC, ethertype, payload)
- Frame construction for TX
- Ethertype constants (ARP 0x0806, IPv4 0x0800)

**Module 5: ARP Protocol** (`src/net/arp.rs`)
- ARP packet structure (hardware type, protocol type, operation)
- Parse ARP requests/replies
- Generate ARP replies for our IP
- Simple ARP cache (Vec<ArpEntry>)
- ARP request generation (for future use)

### Implementation Strategy

**Step 1: GENET Controller Initialization**
- [ ] Find MMIO base address for GENET (verify in BCM2711 docs)
- [ ] Implement soft reset sequence
- [ ] Configure UMAC (Unimac - MAC sublayer)
- [ ] Set MAC address registers
- [ ] Set up simple RX/TX buffers (avoid DMA descriptors initially)
- [ ] Configure interrupts and register with GIC

**Step 2: MDIO Bus Setup**
- [ ] Initialize MDIO controller within GENET
- [ ] Implement MDIO read operation (with timeout)
- [ ] Implement MDIO write operation (with timeout)
- [ ] Test by reading PHY ID registers

**Step 3: PHY Configuration**
- [ ] Detect PHY at MDIO address (scan if needed)
- [ ] Read and verify PHY ID registers
- [ ] Soft reset PHY
- [ ] Configure auto-negotiation (advertise capabilities)
- [ ] Wait for link up
- [ ] Read negotiated speed/duplex

**Step 4: Frame RX/TX**
- [ ] Implement transmit function (write to TX buffer, trigger send)
- [ ] Implement receive polling (check RX status)
- [ ] Implement receive interrupt handler
- [ ] Parse Ethernet frame header
- [ ] Handle frame dispatch by ethertype

**Step 5: ARP Implementation**
- [ ] Define ARP packet structures
- [ ] Parse incoming ARP requests
- [ ] Generate ARP replies
- [ ] Add ARP cache entry on reply
- [ ] Shell command to show ARP cache

### Data Structures (Preliminary)

```rust
// MAC address (6 bytes)
#[derive(Copy, Clone, Debug)]
struct MacAddress([u8; 6]);

// Ethernet frame header (14 bytes)
struct EthernetHeader {
    dest_mac: MacAddress,
    src_mac: MacAddress,
    ethertype: u16,  // Big-endian
}

// Full frame (for simple buffer management)
const MTU: usize = 1500;
const FRAME_SIZE: usize = 14 + MTU; // Header + payload

struct EthernetFrame {
    data: [u8; FRAME_SIZE],
    length: usize,
}

// ARP packet (28 bytes for Ethernet/IPv4)
struct ArpPacket {
    hardware_type: u16,    // 1 = Ethernet
    protocol_type: u16,    // 0x0800 = IPv4
    hw_addr_len: u8,       // 6 for MAC
    proto_addr_len: u8,    // 4 for IPv4
    operation: u16,        // 1 = request, 2 = reply
    sender_mac: MacAddress,
    sender_ip: [u8; 4],
    target_mac: MacAddress,
    target_ip: [u8; 4],
}

// ARP cache entry
struct ArpEntry {
    ip: [u8; 4],
    mac: MacAddress,
    timestamp: u64,  // From system timer, for expiration
}

// GENET controller state
struct GenetController {
    base_addr: usize,
    mac_addr: MacAddress,
    // Simple buffer approach (no descriptors)
    rx_buffer: [u8; 2048],  // Space for one frame
    tx_buffer: [u8; 2048],
}
```

### Shell Commands for Testing

- `eth-status` - Show link up/down, speed, duplex, MAC address
- `eth-stats` - Show RX/TX packet counters
- `eth-send-raw <dest_mac> <hex_data>` - Send raw Ethernet frame
- `arp-cache` - Display ARP cache entries
- `arp-request <ip>` - Send ARP request for IP

### Key Technical Challenges

1. **Timing-Critical Operations**
   - MDIO operations need delays between steps
   - PHY reset requires 10-100ms delay
   - Link auto-negotiation can take 1-3 seconds
   - Need to use system timer for delays

2. **Register Configuration Complexity**
   - GENET has 100+ registers across multiple blocks
   - Initialization order matters
   - Some registers have dependencies
   - Need to consult Linux driver for sequence

3. **Buffer Management**
   - Initial approach: Single RX/TX buffer (simple)
   - Future: Ring buffers with multiple descriptors
   - Need alignment requirements (likely 16 bytes)
   - Handle buffer overflow gracefully

4. **Interrupt Coordination**
   - GENET generates RX/TX interrupts
   - Need to register with existing GIC-400 driver
   - Clear interrupt status correctly
   - Avoid interrupt storms

5. **Frame Validation**
   - Check frame length (64-1518 bytes for standard Ethernet)
   - Verify CRC (or rely on hardware)
   - Filter by destination MAC (unicast to us, broadcast, or multicast)
   - Handle runt frames and oversize frames

### Questions to Research

#### GENET Controller
- [ ] What is the MMIO base address for GENET?
- [ ] What registers control initialization?
- [ ] What is the reset sequence?
- [ ] How are RX/TX buffers configured?
- [ ] What interrupts are available?
- [ ] What is the frame buffer format?

#### BCM54213PE PHY
- [ ] What is the MDIO address of the PHY?
- [ ] What is the PHY ID (for verification)?
- [ ] What registers control link detection?
- [ ] What is the initialization sequence?
- [ ] How does auto-negotiation work?
- [ ] What link speeds are supported?

#### Integration
- [ ] How does firmware/bootloader leave the hardware?
- [ ] Are there any clock configurations needed?
- [ ] What power management is required?

### Known Constants (To Verify)

| Component | Constant | Expected Value | Source Needed |
|-----------|----------|----------------|---------------|
| GENET base | MMIO_BASE | 0xFD580000? | BCM2711 datasheet |
| PHY MDIO addr | PHY_ADDR | 0x01? | Pi 4 schematic |
| PHY ID | PHY_ID | ??? | BCM54213PE datasheet |

---

## Phase 2: Research Findings

### Critical Discovery: Documentation Gaps

**BCM54213PE Datasheet**: ❌ NOT PUBLICLY AVAILABLE
- Broadcom does not publish this PHY datasheet
- Must rely on standard IEEE 802.3 MII registers (0x00-0x1F)
- Vendor-specific registers (0x10-0x1F) may not be documented
- **Workaround**: Use Linux kernel driver code as reference

**GENET Documentation**: ⚠️ LIMITED
- BCM2711 ARM Peripherals PDF exists but has minimal GENET coverage
- No comprehensive register reference like other vendors
- **Primary Source**: Linux kernel driver (`bcmgenet.c`, `bcmgenet.h`)
- **Secondary Source**: U-Boot driver (`bcmgenet.c`)

### Verified Constants

| Component | Constant | Value | Source |
|-----------|----------|-------|--------|
| **GENET base (bus)** | - | `0x7D580000` | Device tree (bcm2711.dtsi) |
| **GENET base (ARM)** | `GENET_BASE` | `0xFD580000` | Bus addr + 0x86000000 offset |
| **GENET size** | - | `0x10000` (64 KB) | Device tree |
| **GENET interrupts** | IRQ 157, 158 | GIC_SPI, level-high | Device tree |
| **PHY MDIO address** | `PHY_ADDR` | `0x01` | Forum posts, dmesg logs |
| **PHY ID** | `PHY_ID` | `0x600D84A2` | Linux driver |
| **MDIO registers offset** | `MDIO_OFF` | `0x0E14` | Device tree mdio node |
| **MDIO registers size** | - | `0x08` | Device tree mdio node |

### GENET Register Map (from Linux kernel)

**Register Block Offsets** (from GENET base):
- `SYS_OFF = 0x0000` - System control registers
- `GR_BRIDGE_OFF = 0x0040` - GR bridge registers
- `EXT_OFF = 0x0080` - Extension block
- `INTRL2_0_OFF = 0x0200` - Interrupt controller 0
- `INTRL2_1_OFF = 0x0240` - Interrupt controller 1
- `RBUF_OFF = 0x0300` - RX buffer control
- `UMAC_OFF = 0x0800` - UniMAC (the actual MAC)
- `TBUF_OFF = 0x0600` - TX buffer control
- `HFB_OFF = 0x8000` - Hardware filter block
- `RDMA_OFF = 0x2000` - RX DMA engine
- `TDMA_OFF = 0x4000` - TX DMA engine

**Key UMAC Registers** (offsets from UMAC_OFF):
- `UMAC_CMD = 0x008` - Command register (TX_EN, RX_EN, etc.)
- `UMAC_MAC0 = 0x00C` - MAC address bytes 0-3
- `UMAC_MAC1 = 0x010` - MAC address bytes 4-5
- `UMAC_MODE = 0x044` - Speed/duplex configuration
- `UMAC_MDIO_CMD = 0x614` - MDIO command/data register
- `UMAC_MIB_CTRL = 0x580` - MIB counter control

**MDIO Command Register Bits** (UMAC_MDIO_CMD):
- Bit [29]: `MDIO_START_BUSY` - Start operation / operation in progress
- Bit [28]: `MDIO_READ_FAIL` - Read failed
- Bits [27:26]: Operation - `10` = read, `01` = write
- Bits [25:21]: PHY address (5 bits)
- Bits [20:16]: Register address (5 bits)
- Bits [15:0]: Data (read or write)

**Interrupt Masks** (INTRL2_0):
- Bit [23]: `UMAC_IRQ_MDIO_DONE` - MDIO operation complete
- Bit [24]: `UMAC_IRQ_MDIO_ERROR` - MDIO operation error
- (Additional interrupts for RX/TX DMA, etc.)

### PHY Register Map (Standard MII - IEEE 802.3)

**Basic Registers** (all PHYs must implement):
- `MII_BMCR = 0x00` - Basic Mode Control Register
  - Bit [15]: Reset
  - Bit [12]: Auto-negotiation enable
  - Bit [9]: Restart auto-negotiation
- `MII_BMSR = 0x01` - Basic Mode Status Register
  - Bit [5]: Auto-negotiation complete
  - Bit [2]: Link status (1 = link up)
- `MII_PHYSID1 = 0x02` - PHY Identifier 1 (upper 16 bits)
- `MII_PHYSID2 = 0x03` - PHY Identifier 2 (lower 16 bits)
- `MII_ADVERTISE = 0x04` - Auto-negotiation Advertisement
- `MII_LPA = 0x05` - Link Partner Ability
- `MII_CTRL1000 = 0x09` - 1000BASE-T Control
- `MII_STAT1000 = 0x0A` - 1000BASE-T Status

**Expected PHY ID**: `0x600D84A2`
- PHYSID1: `0x600D`
- PHYSID2: `0x84A2`

### Initialization Sequence (from U-Boot driver)

**Phase 1: Controller Setup**
1. Map GENET registers (64 KB at 0xFD580000)
2. Read and verify version from SYS_REV_CTRL register (expect v5)
3. Configure interface mode (RGMII)
4. Soft reset via UMAC_CMD register

**Phase 2: MDIO Setup**
1. Initialize MDIO controller (offset 0x0E14)
2. Test read PHY ID registers via MDIO
3. Verify PHY ID matches 0x600D84A2

**Phase 3: PHY Configuration**
1. Read MII_BMSR to check PHY presence
2. Soft reset PHY via MII_BMCR[15]
3. Configure auto-negotiation advertisement
4. Enable and restart auto-negotiation
5. Poll MII_BMSR[5] for auto-negotiation complete (timeout ~3 seconds)
6. Read MII_LPA and MII_STAT1000 to determine link speed/duplex

**Phase 4: MAC Configuration**
1. Reset UMAC (flush RX/TX buffers with 10µs delays)
2. Disable DMA
3. Set up RX/TX descriptor rings (simple mode initially)
4. Configure UMAC_MODE based on PHY link (speed/duplex)
5. Enable DMA
6. Enable TX and RX via UMAC_CMD

**Phase 5: Enable Data Path**
1. Set MAC address in UMAC_MAC0/MAC1
2. Enable promiscuous mode for testing (or set up MAC filtering)
3. Enable interrupts (MDIO done, RX/TX complete)
4. Start packet processing

### Hardware Quirks Discovered

1. **UMAC_MODE Register Cannot Be Read**
   - Writing works, reading returns garbage
   - Must track state in software

2. **Link Status Interrupts Don't Work**
   - Cannot rely on PHY interrupt for link changes
   - Must poll MII_BMSR periodically

3. **Some Registers Are Write-Once**
   - After hardware reset, certain registers only accept one write
   - Must get configuration right the first time

4. **MDIO Timing**
   - Need proper delays between MDIO operations
   - Must poll MDIO_START_BUSY until clear (timeout ~1ms)

5. **PHY Auto-Negotiation Delay**
   - Can take 1-3 seconds to complete
   - Need to handle gracefully during initialization

### Testing Strategy

**Hardware Testing** (on real Pi 4):
1. Use LED indicators (PHY has link/activity LEDs)
2. Monitor MDIO reads for PHY register values
3. Send test frames and monitor TX counter
4. Use external packet sniffer (Wireshark) to verify frames

**QEMU Testing** (limited support):
⚠️ **QEMU raspi4b does NOT emulate GENET controller**
- QEMU 9.0+ supports raspi4b machine but no Ethernet
- Cannot test in current QEMU environment

**Alternative Testing Approaches**:
1. **Unit tests for data structures**
   - Test MAC address parsing/formatting
   - Test Ethernet frame construction
   - Test ARP packet parsing
   - Can run with `cargo test`

2. **MDIO/PHY register simulation**
   - Mock MMIO reads/writes for testing logic
   - Verify state machine transitions
   - Test timeout handling

3. **Integration tests (hardware required)**
   - Mark as `#[cfg(not(test))]` or `#[ignore]`
   - Document manual testing procedure
   - Run on actual Pi 4 hardware

4. **Loopback mode testing**
   - Some MACs support internal loopback
   - Check if GENET v5 has this capability
   - Can test TX→RX without PHY link

### User Requirements: Test Strategy

**From user feedback:**
- ✅ Tests should work with `cargo test` without interactive shell
- ✅ Network tests should be disabled on CI (no hardware)
- ✅ Use `#[ignore]` or conditional compilation for hardware-dependent tests

**Implementation plan:**
```rust
// Unit tests (always run)
#[cfg(test)]
mod tests {
    #[test]
    fn test_mac_address_parse() { ... }

    #[test]
    fn test_ethernet_frame_build() { ... }
}

// Hardware integration tests (manual only)
#[cfg(test)]
mod hardware_tests {
    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_mdio_phy_detect() { ... }

    #[test]
    #[ignore]
    fn test_link_negotiation() { ... }
}

// Shell commands for interactive testing
// - eth-status: Check link, speed, MAC
// - eth-send: Send raw frames
// - arp-cache: View ARP table
```

---

## References

### Official Documentation
- **BCM2711 ARM Peripherals**: <https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf>
  - Section 1.2: Address map (limited GENET info)
- **IEEE 802.3**: Ethernet standards (MII registers defined in Clause 22)

### Linux Kernel Sources
- **GENET driver**: <https://github.com/torvalds/linux/blob/master/drivers/net/ethernet/broadcom/genet/bcmgenet.c>
- **GENET header**: <https://github.com/torvalds/linux/blob/master/drivers/net/ethernet/broadcom/genet/bcmgenet.h>
- **Device tree**: <https://github.com/raspberrypi/linux/blob/rpi-5.4.y/arch/arm/boot/dts/bcm2711.dtsi>
- **PHY driver commit**: <https://github.com/raspberrypi/linux/commit/360c8e98883f9cd075564be8a7fc25ac0785dee4>

### U-Boot Sources
- **GENET driver**: <https://github.com/u-boot/u-boot/blob/master/drivers/net/bcmgenet.c>

### Other Implementations
- **Circle OS (C++)**: <https://github.com/rsta2/circle/blob/master/lib/bcm54213.cpp>
- **Tianocore EDK2**: <https://github.com/tianocore/edk2-platforms/commit/8f330caf903963aadae92372b3ef0a98335c0931>

### Community Resources
- **Hardware pitfalls**: <https://forums.raspberrypi.com/viewtopic.php?t=349563>
- **Kernel device tree docs**: <https://www.kernel.org/doc/Documentation/devicetree/bindings/net/brcm,bcmgenet.txt>

---

## Phase 3: Final Implementation Plan

Based on research findings, here's the updated implementation strategy:

### Module Structure

```
src/
├── drivers/
│   ├── genet.rs       # GENET MAC controller (main driver)
│   └── mdio.rs        # MDIO bus implementation (or inline in genet.rs)
└── net/
    ├── mod.rs         # Network module exports
    ├── ethernet.rs    # Ethernet frame structures and parsing
    └── arp.rs         # ARP protocol implementation
```

### Implementation Phases (Updated)

#### Phase 1: MMIO Infrastructure & Register Definitions
**Goal**: Set up register access with type safety
**Files**: `src/drivers/genet.rs`

1. Define GENET register offsets as constants
2. Create register accessor functions (read_reg/write_reg)
3. Define bit masks for important registers
4. Add hardware version detection
5. **Tests**: Unit tests for register offset calculations

#### Phase 2: MDIO Bus Implementation
**Goal**: Communicate with PHY chip
**Files**: `src/drivers/genet.rs` or `src/drivers/mdio.rs`

1. Implement MDIO read function (with timeout)
2. Implement MDIO write function (with timeout)
3. Add PHY ID detection
4. Verify PHY ID matches BCM54213PE (0x600D84A2)
5. **Tests**:
   - Unit tests for MDIO command encoding
   - Ignored integration test for PHY detection

#### Phase 3: PHY Configuration
**Goal**: Establish link with auto-negotiation
**Files**: `src/drivers/genet.rs`

1. Define MII register constants
2. Implement PHY soft reset
3. Configure auto-negotiation advertisement
4. Implement link status polling
5. Parse link partner ability (speed/duplex)
6. **Tests**:
   - Unit tests for MII register decoding
   - Ignored integration test for link negotiation

#### Phase 4: GENET MAC Initialization
**Goal**: Initialize MAC controller
**Files**: `src/drivers/genet.rs`

1. Implement UMAC soft reset
2. Configure MAC address registers
3. Set up speed/duplex based on PHY
4. Configure RGMII interface
5. Disable DMA initially (simple mode)
6. **Tests**:
   - Unit tests for MAC address encoding
   - Ignored integration test for initialization sequence

#### Phase 5: Simple TX/RX (Polling, No DMA)
**Goal**: Send and receive single frames
**Files**: `src/drivers/genet.rs`

1. Allocate static RX/TX buffers
2. Implement frame transmission (polling)
3. Implement frame reception (polling)
4. Add frame length validation
5. **Tests**:
   - Ignored integration test for loopback (if supported)
   - Manual testing required

#### Phase 6: Ethernet Frame Handling
**Goal**: Parse and construct Ethernet frames
**Files**: `src/net/ethernet.rs`

1. Define `MacAddress` type with Display/Debug
2. Define `EthernetFrame` structure
3. Implement frame parsing (dest/src MAC, ethertype)
4. Implement frame construction
5. Define ethertype constants (ARP, IPv4, etc.)
6. **Tests**:
   - ✅ Unit tests for MAC address parsing
   - ✅ Unit tests for frame construction
   - ✅ Unit tests for frame validation

#### Phase 7: ARP Protocol
**Goal**: Handle Address Resolution Protocol
**Files**: `src/net/arp.rs`

1. Define `ArpPacket` structure
2. Implement ARP packet parsing
3. Implement ARP reply generation
4. Create simple ARP cache (Vec<ArpEntry>)
5. Handle ARP requests for our IP
6. **Tests**:
   - ✅ Unit tests for ARP packet parsing
   - ✅ Unit tests for ARP reply generation
   - Ignored integration test for ARP exchange

#### Phase 8: Interrupt-Driven RX
**Goal**: Replace polling with interrupts
**Files**: `src/drivers/genet.rs`

1. Register GENET IRQs (157, 158) with GIC
2. Implement interrupt handler
3. Enable RX interrupt in INTRL2_0
4. Clear interrupt status correctly
5. Queue received frames for processing
6. **Tests**:
   - Ignored integration test for interrupt handling

#### Phase 9: Shell Commands
**Goal**: Interactive testing interface
**Files**: `src/shell.rs`, update command parser

1. `eth-init` - Initialize Ethernet driver
2. `eth-status` - Show link status, speed, duplex, MAC
3. `eth-stats` - Show RX/TX packet/byte counters
4. `eth-send <dest_mac> <data>` - Send raw Ethernet frame
5. `arp-cache` - Display ARP cache entries
6. `arp-request <ip>` - Send ARP request
7. **Tests**:
   - ✅ Unit tests for command parsing
   - Manual testing in QEMU/hardware

### Data Structure Refinement

```rust
// src/drivers/genet.rs

/// GENET v5 controller (BCM2711)
pub struct GenetController {
    base_addr: usize,
    mac_addr: MacAddress,
    link_speed: LinkSpeed,
    link_duplex: LinkDuplex,
    rx_buffer: [u8; 2048],
    tx_buffer: [u8; 2048],
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LinkSpeed {
    Speed10,
    Speed100,
    Speed1000,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LinkDuplex {
    Half,
    Full,
}

// src/net/ethernet.rs

/// 48-bit MAC address
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    pub const fn new(bytes: [u8; 6]) -> Self { ... }
    pub const fn broadcast() -> Self { Self([0xFF; 6]) }
    pub fn is_broadcast(&self) -> bool { ... }
    pub fn is_multicast(&self) -> bool { ... }
}

/// Ethernet frame (header + payload)
pub struct EthernetFrame<'a> {
    dest: MacAddress,
    src: MacAddress,
    ethertype: u16,
    payload: &'a [u8],
}

// Ethertype constants
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_IPV6: u16 = 0x86DD;

// src/net/arp.rs

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct ArpPacket {
    hardware_type: u16,     // Network byte order (big-endian)
    protocol_type: u16,
    hw_addr_len: u8,
    proto_addr_len: u8,
    operation: u16,
    sender_mac: MacAddress,
    sender_ip: [u8; 4],
    target_mac: MacAddress,
    target_ip: [u8; 4],
}

pub const ARP_HARDWARE_ETHERNET: u16 = 1;
pub const ARP_PROTOCOL_IPV4: u16 = 0x0800;
pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;
```

### Testing Strategy (Final)

**Always-Run Tests** (`cargo test`):
```rust
// Unit tests for data structures and pure functions
#[cfg(test)]
mod tests {
    #[test]
    fn test_mac_address_display() { ... }

    #[test]
    fn test_ethernet_frame_parse() { ... }

    #[test]
    fn test_arp_packet_encode() { ... }

    #[test]
    fn test_mdio_command_bits() { ... }
}
```

**Hardware Tests** (`cargo test -- --ignored`):
```rust
#[cfg(test)]
mod hardware_tests {
    use crate::drivers::genet::GenetController;

    #[test]
    #[ignore]
    fn test_phy_id_detection() {
        let genet = GenetController::new();
        let phy_id = genet.mdio_read_phy_id().unwrap();
        assert_eq!(phy_id, 0x600D84A2);
    }

    #[test]
    #[ignore]
    fn test_link_negotiation() {
        let mut genet = GenetController::new();
        genet.init().unwrap();
        assert!(genet.wait_for_link(3000).is_ok());
    }
}
```

**Manual Testing** (on hardware):
```
daedalus> eth-init
Initializing GENET v5...
PHY detected: BCM54213PE (ID: 0x600D84A2)
Auto-negotiation in progress...
Link established: 1000 Mbps, Full Duplex
MAC address: B8:27:EB:XX:XX:XX

daedalus> eth-status
Link: UP
Speed: 1000 Mbps
Duplex: Full
RX: 0 packets, 0 bytes
TX: 0 packets, 0 bytes

daedalus> arp-request 192.168.1.1
Sending ARP request for 192.168.1.1...
```

### Critical Implementation Notes

1. **Address Translation**:
   - Device tree uses bus address `0x7D580000`
   - ARM physical address is `0xFD580000`
   - Add constant: `GENET_BASE = 0xFD58_0000`

2. **Byte Order**:
   - Network protocols are big-endian
   - ARM is little-endian
   - Use `u16::to_be()` / `u16::from_be()` for ethertype, ARP fields

3. **Timing**:
   - MDIO operations: Poll with ~1ms timeout
   - PHY reset: 10-100ms delay
   - Auto-negotiation: Up to 3 seconds
   - Use existing `timer::delay_us()` and `timer::delay_ms()`

4. **Error Handling**:
   - MDIO timeout: Return error, don't panic
   - Link negotiation failure: Return error with details
   - Invalid frame: Drop and log, don't crash

5. **Safety**:
   - MMIO accesses are `unsafe`
   - Use volatile reads/writes
   - Document SAFETY for each unsafe block

### Success Criteria

**Milestone #12 is complete when:**
- ✅ GENET controller initializes without errors
- ✅ PHY ID detection succeeds (0x600D84A2)
- ✅ Link auto-negotiation completes
- ✅ Can send raw Ethernet frames
- ✅ Can receive Ethernet frames (polling or interrupt)
- ✅ Can respond to ARP requests
- ✅ All unit tests pass (`cargo test`)
- ✅ Manual testing on hardware succeeds
- ✅ Documentation updated (hardware doc + roadmap)

### Documentation Requirements

After implementation, create:
1. `docs/src/hardware/genet.md` - GENET hardware reference
   - Register map
   - Initialization sequence
   - MDIO protocol
   - Quirks and workarounds

2. `docs/src/hardware/bcm54213pe.md` - PHY reference
   - MII registers
   - Auto-negotiation
   - Link status detection

3. Update `docs/src/hardware/memory-map.md` - Add GENET base address

4. Update `docs/src/roadmap.md` - Mark Milestone #12 complete

### Estimated Complexity

- **Phase 1-2** (MMIO + MDIO): 2-3 hours
- **Phase 3-4** (PHY + MAC init): 3-4 hours
- **Phase 5** (Simple TX/RX): 2-3 hours
- **Phase 6-7** (Ethernet + ARP): 2-3 hours
- **Phase 8-9** (IRQ + Shell): 2-3 hours
- **Testing + Debugging**: 4-6 hours
- **Documentation**: 2-3 hours

**Total**: ~20-25 hours of focused work

---

## Ready to Begin Implementation

Research phase complete! All critical information gathered:
- ✅ Register addresses verified
- ✅ Initialization sequence documented
- ✅ Hardware quirks identified
- ✅ Testing strategy defined
- ✅ Implementation plan finalized

**Next step**: Begin Phase 1 implementation (MMIO infrastructure)

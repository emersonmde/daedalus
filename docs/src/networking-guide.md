# Networking Guide

**Complete guide to networking in DaedalusOS**
**Status**: Foundation complete, ready for TX/RX implementation
**Last Updated**: 2025-11-09 (Milestone #12)

---

## Overview

This guide provides a complete reference for working with DaedalusOS's network stack. It covers everything from low-level hardware drivers to high-level protocol handling.

### Quick Navigation

| Topic | Documentation | Code |
|-------|---------------|------|
| **Hardware** | [GENET Driver](hardware/genet.md) | `src/drivers/net/ethernet/broadcom/genet.rs` |
| **Protocols** | [Ethernet & ARP](architecture/networking.md) | `src/net/` |
| **Testing** | This document (below) | `cargo test` |
| **Integration** | This document (below) | `src/shell.rs` |

---

## Architecture Overview

### Component Map

```
┌──────────────────────────────────────────────────────────────┐
│                      User Application                         │
│                   (Future: HTTP, GPIO API)                    │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────────┐
│                      Protocol Handlers                        │
│         • ARP Responder (src/net/arp.rs)                     │
│         • Future: IP, TCP, UDP (via smoltcp)                 │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────────┐
│                    Frame Processing                           │
│         • Ethernet Frame Parser (src/net/ethernet.rs)        │
│         • Protocol Dispatch (by EtherType)                   │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────────┐
│                     GENET Driver                              │
│         • Register Control (src/drivers/genet.rs)            │
│         • MDIO Bus Protocol                                  │
│         • PHY Management (BCM54213PE)                        │
│         • TX/RX Buffers (future)                             │
│         • Interrupt Handling (future)                        │
└───────────────────────────┬──────────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────────┐
│                      Hardware                                 │
│    BCM2711 SoC          BCM54213PE PHY       Ethernet Port   │
│    (GENET MAC)          (Physical Layer)     (RJ45)          │
└──────────────────────────────────────────────────────────────┘
```

### Data Flow

#### Transmit Path (Future)

```
Application
    │
    └─► Create message (e.g., HTTP response)
         │
         └─► TCP layer (smoltcp)
              │
              └─► IP layer (smoltcp)
                   │
                   └─► ARP lookup (get dest MAC)
                        │
                        └─► Ethernet frame construction
                             │
                             └─► GENET TX
                                  │
                                  └─► PHY chip
                                       │
                                       └─► Network wire
```

#### Receive Path (Future)

```
Network wire
    │
    └─► PHY chip
         │
         └─► GENET RX (interrupt)
              │
              └─► Ethernet frame parsing
                   │
                   ├─► EtherType 0x0806 → ARP handler
                   │    └─► Process request, send reply
                   │
                   ├─► EtherType 0x0800 → IP handler (smoltcp)
                   │    └─► TCP/UDP processing
                   │         └─► Application handler
                   │
                   └─► Unknown → Drop and log
```

---

## Current Implementation Status

### ✅ Completed (Milestone #12)

**Hardware Layer**:
- GENET v5 controller register definitions
- MMIO read/write infrastructure
- MDIO protocol implementation (read/write PHY registers)
- PHY detection and ID verification (BCM54213PE)
- Hardware diagnostics system
- Safe hardware presence checking (QEMU compatibility)

**Protocol Layer**:
- MAC address type with validation
- Ethernet II frame parsing and construction
- ARP packet parsing and construction
- ARP request/reply generation
- Network byte order handling

**Testing**:
- 65 total unit tests passing
- 30 network protocol tests
- 4 GENET driver tests
- All tests run in QEMU

**Documentation**:
- Complete hardware reference ([GENET](hardware/genet.md))
- Complete protocol guide ([Networking](architecture/networking.md))
- This integration guide
- Milestone summary with test results

### ❌ Not Yet Implemented

**Hardware Layer** (Milestone #13):
- Frame transmission (TX path)
- Frame reception (RX path)
- DMA engine configuration
- Interrupt-driven RX/TX
- MAC address configuration
- Link state monitoring

**Protocol Layer** (Milestone #14-16):
- ARP cache management
- ARP request timeout/retry
- IPv4 protocol
- ICMP (ping)
- TCP/UDP (via smoltcp)
- DHCP client

**Application Layer** (Milestone #17):
- HTTP server
- GPIO control API
- DNS client

---

## Getting Started

### Prerequisites

- Raspberry Pi 4 Model B (BCM2711)
- Ethernet cable
- Network with connectivity (for testing)
- Serial console or monitor for debugging

### Building

```bash
# Build kernel
cargo build --release

# Run tests
cargo test

# Build documentation
cargo doc --open
```

### Running on QEMU

```bash
# Interactive shell (no network hardware)
cargo run

# Run diagnostics (will report no hardware)
daedalus> eth-diag
[INFO] Hardware not present (running in QEMU?)
```

**Note**: QEMU 9.0's `raspi4b` machine does not emulate GENET. Network testing requires real hardware.

### Running on Raspberry Pi 4

1. **Build kernel**:
   ```bash
   cargo build --release
   ```

2. **Copy to SD card**:
   ```bash
   cp target/aarch64-daedalus/release/daedalus /path/to/sd/kernel8.img
   ```

3. **Boot and test**:
   ```
   daedalus> eth-diag
   [PASS] GENET v5.2.16 detected
   [PASS] PHY found at address 1: BCM54213PE
   [PASS] Link status: UP
   ```

---

## Working with the GENET Driver

### Basic Usage

```rust
use daedalus::drivers::genet::GenetController;

// Create controller instance
let genet = GenetController::new();

// Check if hardware is present (safe in QEMU)
if !genet.is_present() {
    println!("No GENET hardware detected");
    return;
}

// Get hardware version
let version = genet.get_version();
println!("GENET version: {:#010X}", version);
```

### MDIO Operations

```rust
use daedalus::drivers::genet::{MII_BMSR, MII_PHYSID1, MII_PHYSID2};

// Read PHY ID
let id1 = genet.mdio_read(PHY_ADDR, MII_PHYSID1)?;
let id2 = genet.mdio_read(PHY_ADDR, MII_PHYSID2)?;
let phy_id = ((id1 as u32) << 16) | (id2 as u32);

println!("PHY ID: {:#010X}", phy_id); // Should be 0x600D84A2

// Read link status
let bmsr = genet.mdio_read(PHY_ADDR, MII_BMSR)?;
let link_up = (bmsr & BMSR_LSTATUS) != 0;
println!("Link: {}", if link_up { "UP" } else { "DOWN" });

// Write to PHY register (example: software reset)
genet.mdio_write(PHY_ADDR, MII_BMCR, BMCR_RESET);
```

### Running Diagnostics

```rust
// Run comprehensive hardware check
let success = genet.diagnostic();

if success {
    println!("Hardware ready for network operations");
} else {
    println!("Hardware issues detected, see output above");
}
```

**See**: [GENET Hardware Reference](hardware/genet.md) for complete register documentation.

---

## Working with Ethernet Frames

### Sending a Frame (Conceptual - TX not yet implemented)

```rust
use daedalus::net::ethernet::*;

// Create frame
let frame = EthernetFrame::new(
    MacAddress::broadcast(),              // Destination
    MacAddress::new([0xB8, 0x27, 0xEB, 1, 2, 3]), // Source (our MAC)
    ETHERTYPE_ARP,                         // Protocol
    &payload_data,                         // Payload
);

// Serialize to buffer
let mut buffer = [0u8; 1518];
let size = frame.write_to(&mut buffer).unwrap();

// Send via GENET (future)
// genet.transmit(&buffer[..size])?;
```

### Receiving a Frame (Conceptual - RX not yet implemented)

```rust
// Receive raw bytes from GENET (future)
// let raw_frame = genet.receive()?;

// Parse frame
if let Some(frame) = EthernetFrame::parse(&raw_frame) {
    println!("From: {}", frame.src_mac);
    println!("To: {}", frame.dest_mac);
    println!("Protocol: {:#06X}", frame.ethertype);

    // Dispatch by protocol
    match frame.ethertype {
        ETHERTYPE_ARP => handle_arp(frame.payload),
        ETHERTYPE_IPV4 => handle_ipv4(frame.payload),
        _ => println!("Unknown protocol"),
    }
}
```

**See**: [Ethernet Protocol Guide](architecture/networking.md#ethernet-ii-frames) for complete API reference.

---

## Working with ARP

### Sending an ARP Request

```rust
use daedalus::net::arp::*;
use daedalus::net::ethernet::*;

fn send_arp_request(target_ip: [u8; 4]) {
    // Our network configuration
    let our_mac = MacAddress::new([0xB8, 0x27, 0xEB, 1, 2, 3]);
    let our_ip = [192, 168, 1, 100];

    // Create ARP request
    let arp = ArpPacket::request(our_mac, our_ip, target_ip);

    // Serialize ARP packet
    let mut arp_buffer = [0u8; 28];
    arp.write_to(&mut arp_buffer).unwrap();

    // Wrap in Ethernet frame (broadcast)
    let frame = EthernetFrame::new(
        MacAddress::broadcast(),
        our_mac,
        ETHERTYPE_ARP,
        &arp_buffer,
    );

    // Serialize and send (future)
    let mut frame_buffer = [0u8; 64];
    let size = frame.write_to(&mut frame_buffer).unwrap();
    // genet.transmit(&frame_buffer[..size])?;
}
```

### Handling ARP Requests

```rust
fn handle_arp_request(arp: &ArpPacket, our_mac: MacAddress, our_ip: [u8; 4]) {
    // Is this request for our IP?
    if arp.operation == ArpOperation::Request && arp.target_ip == our_ip {
        // Create reply
        let reply = ArpPacket::reply(
            our_mac,           // We are the sender
            our_ip,
            arp.sender_mac,    // They are the target
            arp.sender_ip,
        );

        // Serialize and send (future)
        let mut buffer = [0u8; 28];
        reply.write_to(&mut buffer).unwrap();

        // Wrap in Ethernet frame (unicast to requester)
        let frame = EthernetFrame::new(
            arp.sender_mac,  // Direct reply
            our_mac,
            ETHERTYPE_ARP,
            &buffer,
        );

        // Send via GENET (future)
        // send_frame(&frame)?;
    }
}
```

**See**: [ARP Protocol Guide](architecture/networking.md#arp-address-resolution-protocol) for complete examples.

---

## Shell Commands

### `eth-diag` - Ethernet Diagnostics

Run comprehensive hardware diagnostics.

**Usage**:
```
daedalus> eth-diag
```

**Output on Real Pi 4**:
```
[DIAG] Ethernet Hardware Diagnostics
[DIAG] ================================
[DIAG] Step 1: GENET Controller Detection
[DIAG]   Reading SYS_REV_CTRL @ 0xFD580000...
[PASS]   GENET v5.2.16 detected (version: 0x00050210)

[DIAG] Step 2: PHY Detection
[DIAG]   Scanning MDIO address 1...
[DIAG]   Reading PHY_ID1 @ addr 1, reg 0x02...
[DIAG]     Value: 0x600D
[DIAG]   Reading PHY_ID2 @ addr 1, reg 0x03...
[DIAG]     Value: 0x84A2
[PASS]   PHY found at address 1: BCM54213PE (ID: 0x600D84A2)

[DIAG] Step 3: PHY Status
[DIAG]   Reading BMSR (Basic Mode Status Register)...
[DIAG]     BMSR: 0x7949
[DIAG]       Link status: UP
[DIAG]       Auto-negotiation: COMPLETE
[DIAG]   Reading BMCR (Basic Mode Control Register)...
[DIAG]     BMCR: 0x1140
[DIAG]       Auto-negotiation: ENABLED

[PASS] ================================
[PASS] Hardware diagnostics complete!
[PASS] GENET v5 and BCM54213PE PHY detected
```

**Output in QEMU**:
```
[DIAG] Ethernet Hardware Diagnostics
[DIAG] ================================
[DIAG] Step 1: GENET Controller Detection
[DIAG]   Reading SYS_REV_CTRL @ 0xFD580000...
[INFO]   Hardware not present (running in QEMU?)
[SKIP] Diagnostics completed (no hardware detected)
```

**Implementation**: `src/shell.rs` (line ~200), `src/drivers/net/ethernet/broadcom/genet.rs` (line ~373)

### Future Commands (Planned)

- `eth-status` - Show link status, speed, duplex, MAC address
- `eth-stats` - Display RX/TX packet/byte counters
- `eth-send <dest_mac> <data>` - Send raw Ethernet frame
- `arp-cache` - Display ARP cache entries
- `arp-request <ip>` - Send ARP request for an IP address
- `ping <ip>` - Send ICMP echo request
- `dhcp` - Request IP via DHCP

---

## Testing

### Running Tests

```bash
# All tests
cargo test

# Only network tests
cargo test --lib net

# Only GENET tests
cargo test --lib drivers::genet

# Specific test
cargo test test_mac_address_display

# Show test output
cargo test -- --nocapture
```

### Test Organization

**Unit Tests** (run in QEMU):
- `src/net/ethernet.rs` - 18 tests
- `src/net/arp.rs` - 12 tests
- `src/drivers/genet.rs` - 4 tests

**Integration Tests** (future, require hardware):
- Marked with `#[ignore]`
- Run with `cargo test -- --ignored`

**Manual Tests** (on hardware):
- Use shell commands
- Capture with Wireshark on connected network
- Verify with external tools

### Test Coverage

| Component | Unit Tests | Integration Tests | Manual Tests |
|-----------|------------|-------------------|--------------|
| MAC Address | ✅ 12 tests | N/A | N/A |
| Ethernet Frames | ✅ 6 tests | ❌ Planned | ❌ Planned |
| ARP Packets | ✅ 12 tests | ❌ Planned | ❌ Planned |
| GENET Registers | ✅ 4 tests | N/A | N/A |
| MDIO Protocol | ❌ Mock only | ❌ Planned | ✅ `eth-diag` |
| PHY Detection | ❌ Mock only | ❌ Planned | ✅ `eth-diag` |
| Frame TX/RX | ❌ Not impl | ❌ Planned | ❌ Planned |

### Example Test

```rust
#[test_case]
fn test_arp_request_creation() {
    let our_mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
    let our_ip = [192, 168, 1, 100];
    let target_ip = [192, 168, 1, 1];

    let request = ArpPacket::request(our_mac, our_ip, target_ip);

    assert_eq!(request.operation, ArpOperation::Request);
    assert_eq!(request.sender_mac, our_mac);
    assert_eq!(request.sender_ip, our_ip);
    assert_eq!(request.target_mac, MacAddress::zero());
    assert_eq!(request.target_ip, target_ip);
}
```

---

## Debugging

### QEMU Debugging

Since QEMU doesn't emulate GENET, debugging focuses on protocol logic:

```rust
// Use unit tests to verify protocol handling
cargo test test_ethernet_frame_roundtrip

// Test serialization manually
let frame = EthernetFrame::new(/* ... */);
let mut buffer = [0u8; 1518];
let size = frame.write_to(&mut buffer).unwrap();

// Dump hex
for (i, byte) in buffer[..size].iter().enumerate() {
    if i % 16 == 0 {
        println!();
        print!("{:04X}: ", i);
    }
    print!("{:02X} ", byte);
}
```

### Hardware Debugging

**Step 1: Verify Hardware Detection**
```
daedalus> eth-diag
```

Check for:
- GENET version matches v5.x.x
- PHY ID matches 0x600D84A2
- Link status shows UP (cable connected)
- Auto-negotiation completes

**Step 2: Monitor PHY Registers**

```rust
// Read key PHY registers
let bmsr = genet.mdio_read(1, MII_BMSR)?;
let bmcr = genet.mdio_read(1, MII_BMCR)?;

println!("BMSR: {:#06X}", bmsr);
println!("  Link: {}", if (bmsr & 0x04) != 0 { "UP" } else { "DOWN" });
println!("  AN Complete: {}", if (bmsr & 0x20) != 0 { "YES" } else { "NO" });

println!("BMCR: {:#06X}", bmcr);
println!("  AN Enabled: {}", if (bmcr & 0x1000) != 0 { "YES" } else { "NO" });
```

**Step 3: Packet Capture (Future)**

Once TX/RX is implemented, use Wireshark on a connected device:

```bash
# On a Linux machine connected to the Pi 4
sudo tcpdump -i eth0 -w capture.pcap

# Or use Wireshark GUI
wireshark
```

Filter for:
- ARP: `arp`
- From Pi's MAC: `eth.src == b8:27:eb:xx:xx:xx`
- To Pi's MAC: `eth.dst == b8:27:eb:xx:xx:xx`

### Common Issues

**Issue**: `eth-diag` reports no hardware in QEMU
- **Cause**: QEMU 9.0 doesn't emulate GENET
- **Solution**: This is expected. Test on real Pi 4.

**Issue**: PHY ID mismatch
- **Cause**: Different PHY chip (not Pi 4?) or MDIO issue
- **Solution**: Verify hardware, check MDIO timing

**Issue**: Link status DOWN
- **Cause**: Cable unplugged, bad cable, switch port down
- **Solution**: Check cable, try different switch port

**Issue**: Auto-negotiation timeout
- **Cause**: PHY configuration issue or partner doesn't support auto-neg
- **Solution**: Check BMCR/BMSR registers, verify cable/switch

**Issue**: Frames not received
- **Cause**: MAC filtering, promiscuous mode not enabled, interrupt not firing
- **Solution**: Check UMAC_CMD settings, verify interrupt registration

---

## Network Configuration

### MAC Address

The Raspberry Pi 4 has a factory-programmed MAC address stored in OTP (One-Time Programmable) memory. Our driver currently reads this from device-specific registers (future implementation).

**Temporary**: Hard-code MAC address during development:
```rust
const OUR_MAC: MacAddress = MacAddress([0xB8, 0x27, 0xEB, 0x01, 0x02, 0x03]);
```

**Production**: Read from OTP:
```rust
// Future implementation
let mac = genet.read_mac_address();
```

### IP Address

**Static IP** (current approach):
```rust
const OUR_IP: [u8; 4] = [192, 168, 1, 100];
```

**DHCP** (future):
```rust
// Use smoltcp's DHCP client
let ip = dhcp_client.request_ip()?;
```

### Network Settings

Typical development network configuration:

| Setting | Value | Configurable |
|---------|-------|--------------|
| **MAC Address** | Read from OTP | ❌ (hardware) |
| **IP Address** | 192.168.1.100 | ✅ (code constant) |
| **Netmask** | 255.255.255.0 | ✅ (future) |
| **Gateway** | 192.168.1.1 | ✅ (future) |
| **DNS** | 192.168.1.1 | ✅ (future) |
| **Link Speed** | Auto-negotiated | ❌ (PHY handles) |
| **Duplex** | Auto-negotiated | ❌ (PHY handles) |

---

## Performance Considerations

### MDIO Timing

MDIO operations are relatively slow (~1ms each):
- **PHY ID read**: 2 MDIO reads = ~2ms
- **Link status poll**: 1 MDIO read = ~1ms
- **Auto-negotiation**: Can take 1-3 seconds

**Optimization**: Don't poll PHY registers in performance-critical paths. Cache link state and update periodically.

### Frame Processing

**Future Bottlenecks**:
- Copying data between buffers (use zero-copy where possible)
- Protocol parsing overhead (optimize hot paths)
- Interrupt frequency (tune interrupt coalescing)

**DMA vs. Polling**:
- Polling: Simple, good for low traffic
- DMA: Essential for high traffic (1 Gbps = ~1.5M packets/sec)

### Memory Usage

Current allocations:
- GENET driver: Minimal (no buffers yet)
- Ethernet frames: Stack-allocated or passed by reference
- ARP packets: 28 bytes (stack)

Future allocations:
- RX buffer ring: ~32 KB (16 descriptors × 2 KB)
- TX buffer ring: ~32 KB
- ARP cache: ~1 KB (typical: 64 entries)

---

## Roadmap

### Milestone #13: Frame TX/RX

**Goal**: Send and receive Ethernet frames

**Implementation**:
- Configure GENET TX/RX buffers (simple mode, no DMA)
- Implement `transmit(&[u8])` function
- Implement `receive() -> Option<&[u8]>` function (polling)
- Test with raw frame send/receive

**Verification**:
- Send ARP request from Pi
- Receive ARP request on Pi
- View frames in Wireshark

### Milestone #14: Interrupt-Driven RX

**Goal**: Replace polling with interrupts

**Implementation**:
- Register GENET IRQs (157, 158) with GIC
- Implement RX interrupt handler
- Queue received frames for processing
- Clear interrupt status correctly

**Verification**:
- Receive frames without polling
- Measure latency improvement

### Milestone #15: ARP Responder

**Goal**: Respond to ARP requests

**Implementation**:
- ARP cache with expiration
- ARP request/reply handling
- Integration with RX path

**Verification**:
- `ping 192.168.1.100` from another device
- Pi responds to ARP, then to ICMP (need Milestone #16)

### Milestone #16: TCP/IP Stack (smoltcp)

**Goal**: Full TCP/IP support

**Implementation**:
- Integrate smoltcp crate
- Implement Device trait (maps to GENET)
- Configure IP, routing, sockets
- DHCP client

**Verification**:
- Obtain IP via DHCP
- Ping external hosts
- TCP connection (HTTP GET)

### Milestone #17: HTTP Server

**Goal**: Web-based GPIO control

**Implementation**:
- Simple HTTP server using smoltcp
- REST API for GPIO control
- JSON responses

**Verification**:
- `curl http://192.168.1.100/gpio/21/on`
- LED turns on

---

## API Reference

### Key Types

```rust
// Hardware
pub struct GenetController { /* ... */ }

// Network
pub struct MacAddress(pub [u8; 6]);
pub struct EthernetFrame<'a> { /* ... */ }
pub struct ArpPacket { /* ... */ }
pub enum ArpOperation { Request = 1, Reply = 2 }

// Constants
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_IPV6: u16 = 0x86DD;
```

### Key Functions

```rust
// GENET
impl GenetController {
    pub fn new() -> Self;
    pub fn is_present(&self) -> bool;
    pub fn get_version(&self) -> u32;
    pub fn mdio_read(&self, phy_addr: u8, reg_addr: u8) -> Option<u16>;
    pub fn mdio_write(&self, phy_addr: u8, reg_addr: u8, value: u16) -> bool;
    pub fn read_phy_id(&self) -> Option<u32>;
    pub fn diagnostic(&self) -> bool;
}

// Ethernet
impl EthernetFrame<'_> {
    pub fn new(dest_mac: MacAddress, src_mac: MacAddress,
               ethertype: u16, payload: &[u8]) -> Self;
    pub fn parse(buffer: &[u8]) -> Option<Self>;
    pub fn write_to(&self, buffer: &mut [u8]) -> Option<usize>;
}

// ARP
impl ArpPacket {
    pub fn new(operation: ArpOperation, sender_mac: MacAddress,
               sender_ip: [u8; 4], target_mac: MacAddress,
               target_ip: [u8; 4]) -> Self;
    pub fn request(sender_mac: MacAddress, sender_ip: [u8; 4],
                   target_ip: [u8; 4]) -> Self;
    pub fn reply(sender_mac: MacAddress, sender_ip: [u8; 4],
                 target_mac: MacAddress, target_ip: [u8; 4]) -> Self;
    pub fn parse(buffer: &[u8]) -> Option<Self>;
    pub fn write_to(&self, buffer: &mut [u8]) -> Option<usize>;
}
```

Complete API documentation: `cargo doc --open`

---

## FAQ

**Q: Why doesn't networking work in QEMU?**
A: QEMU 9.0's `raspi4b` machine doesn't fully emulate the GENET controller. Network testing requires real Pi 4 hardware.

**Q: Can I use a different Ethernet PHY?**
A: The driver is specific to BCM54213PE (Pi 4's PHY). Porting would require changes to PHY initialization and MDIO addressing.

**Q: What about Wi-Fi?**
A: Wi-Fi is much more complex (separate driver, firmware, WPA supplicant). Ethernet is the priority for now.

**Q: Why not use smoltcp from the start?**
A: Understanding the hardware first makes debugging easier. We'll integrate smoltcp once TX/RX works.

**Q: How do I capture packets for debugging?**
A: Connect the Pi 4 to a network with another device running Wireshark or tcpdump. The Pi will be visible as a network node.

**Q: What's the maximum throughput?**
A: Hardware supports Gigabit (1000 Mbps). Actual throughput depends on:
- DMA configuration (required for high speed)
- CPU overhead (interrupt handling, context switches)
- Buffer management (zero-copy techniques)
- Realistic target: 100-500 Mbps with simple implementation

**Q: Can I test without a network cable?**
A: Loopback mode (if supported by GENET) would allow testing TX→RX internally. This is not yet implemented.

---

## Further Reading

### Official Documentation
- [GENET Hardware Reference](hardware/genet.md)
- [Ethernet & ARP Protocol Guide](architecture/networking.md)
- [Memory Map](hardware/memory-map.md)

### External Resources
- **RFC 826**: ARP Protocol - <https://www.rfc-editor.org/rfc/rfc826>
- **IEEE 802.3**: Ethernet Standards
- **smoltcp**: <https://github.com/smoltcp-rs/smoltcp>
- **Linux GENET Driver**: <https://github.com/torvalds/linux/tree/master/drivers/net/ethernet/broadcom/genet>

### Community
- **Raspberry Pi Forums**: <https://forums.raspberrypi.com/>
- **OSDev Wiki**: <https://wiki.osdev.org/Network_Stack>

---

## Contributing

When working on network code:

1. **Read the relevant documentation** (hardware or protocol guide)
2. **Write tests first** (if possible)
3. **Verify constants** from datasheets or RFCs
4. **Document sources** in code comments
5. **Test on hardware** (not just QEMU)
6. **Capture packets** for verification

Example code comment:
```rust
// MDIO Read operation bits (bits 27:26 = 0b10)
// Source: Linux kernel bcmgenet.h, line 487
const MDIO_RD: u32 = 2 << 26;
```

---

**Last Updated**: 2025-11-09 (Milestone #12 Complete)
**Next Milestone**: #13 - Frame TX/RX Implementation

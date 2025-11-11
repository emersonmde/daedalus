# Network Protocol Stack

**Modules**: `src/net/ethernet.rs`, `src/net/arp.rs`, `src/drivers/netdev.rs`
**Status**: Protocol parsing and device abstraction implemented
**Testing**: 66 unit tests passing (30 protocol + 36 other)

---

## Overview

DaedalusOS implements a lightweight network protocol stack for Ethernet networking. The current implementation includes:

- **Device Abstraction**: `NetworkDevice` trait for hardware portability
- **Layer 2 Protocols**: Ethernet II frames and ARP
- **GENET Driver**: BCM2711 Ethernet controller (Pi 4)

This provides the foundation for future IP/TCP/UDP support via smoltcp.

### Architecture Layers

```
┌────────────────────────────────────────┐
│      Application Layer                 │
│  (Future: HTTP, DNS, DHCP, etc.)       │
└────────────────┬───────────────────────┘
                 │
┌────────────────┴───────────────────────┐
│      Transport Layer                   │
│  (Future: TCP, UDP via smoltcp)        │
└────────────────┬───────────────────────┘
                 │
┌────────────────┴───────────────────────┐
│      Network Layer                     │
│  (Future: IPv4, IPv6, ICMP)            │
└────────────────┬───────────────────────┘
                 │
┌────────────────┴───────────────────────┐
│      Data Link Layer (CURRENT)         │
│  • Ethernet II Frames                  │  ← src/net/ethernet.rs
│  • ARP (Address Resolution)            │  ← src/net/arp.rs
└────────────────┬───────────────────────┘
                 │
┌────────────────┴───────────────────────┐
│      Physical Layer                    │
│  • GENET MAC Controller                │  ← src/drivers/genet.rs
│  • BCM54213PE PHY Chip                 │
└────────────────────────────────────────┘
```

### Current Implementation Scope

**✅ Implemented**:
- **Device Abstraction**: `NetworkDevice` trait for multiple hardware implementations
- **Hardware Driver**: GENET v5 controller (Pi 4) with trait implementation
- Ethernet II frame parsing and construction
- MAC address representation and validation
- ARP packet parsing and construction
- ARP request/reply generation
- Network byte order handling (big-endian)

**❌ Not Yet Implemented** (Coming in Milestone #13+):
- Actual frame transmission/reception (hardware TX/RX)
- ARP cache management
- IP protocol (IPv4/IPv6)
- Transport protocols (TCP/UDP via smoltcp)
- Application protocols

---

## Network Device Abstraction

Module: `src/drivers/netdev.rs`

The `NetworkDevice` trait provides a hardware-independent interface for Ethernet network devices. This abstraction enables:

- **Hardware portability**: Support multiple Ethernet controllers (Pi 4 GENET, future Pi 5, QEMU mock)
- **Testing**: Mock devices for protocol testing without hardware
- **smoltcp integration**: Clean interface for TCP/IP stack (Milestone #16)

See [ADR-003: Network Device Abstraction](../decisions/adr-003-network-device-trait.md) for design rationale.

### NetworkDevice Trait

```rust
pub trait NetworkDevice {
    /// Check if hardware is present (false in QEMU)
    fn is_present(&self) -> bool;

    /// Initialize device (reset MAC, configure PHY, set up buffers)
    fn init(&mut self) -> Result<(), NetworkError>;

    /// Transmit Ethernet frame (blocking, 60-1514 bytes)
    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError>;

    /// Receive frame (non-blocking, returns None if no frame available)
    fn receive(&mut self) -> Option<&[u8]>;

    /// Get device MAC address
    fn mac_address(&self) -> MacAddress;

    /// Check link status (optional, default: false)
    fn link_up(&self) -> bool { false }
}
```

### Error Handling

```rust
pub enum NetworkError {
    HardwareNotPresent,   // Device not detected
    NotInitialized,       // init() not called yet
    TxBufferFull,         // Hardware TX queue full
    FrameTooLarge,        // Frame > 1514 bytes
    FrameTooSmall,        // Frame < 60 bytes
    HardwareError,        // MAC/PHY error
    Timeout,              // Operation timeout
    InvalidConfiguration, // Bad parameters
}
```

### Current Implementations

#### GenetController (Raspberry Pi 4)

```rust
use daedalus::drivers::genet::GenetController;
use daedalus::drivers::netdev::NetworkDevice;

let mut netdev = GenetController::new();

// Check hardware presence (returns false in QEMU)
if netdev.is_present() {
    netdev.init()?;

    // Get MAC address
    let mac = netdev.mac_address();

    // Check link status (reads PHY BMSR register)
    if netdev.link_up() {
        // Transmit frame (Milestone #13)
        netdev.transmit(&frame)?;

        // Receive frame (Milestone #13)
        if let Some(frame) = netdev.receive() {
            // Process frame
        }
    }
}
```

**Hardware**: BCM2711 GENET v5 Ethernet MAC controller
**PHY**: BCM54213PE Gigabit Ethernet transceiver

#### MockNetworkDevice (Future - Milestone #14)

Planned mock implementation for QEMU testing:

```rust
pub struct MockNetworkDevice {
    rx_queue: Vec<Vec<u8>>,       // Injected RX frames
    tx_captured: Vec<Vec<u8>>,    // Captured TX frames
    mac: MacAddress,
}

impl NetworkDevice for MockNetworkDevice {
    fn is_present(&self) -> bool { true }  // Always present

    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError> {
        self.tx_captured.push(frame.to_vec());  // Capture for testing
        Ok(())
    }

    fn receive(&mut self) -> Option<&[u8]> {
        self.rx_queue.pop().map(|frame| frame.as_slice())
    }
}
```

This will enable network protocol testing in QEMU without real hardware.

### Design Decisions

**Why blocking transmit?**
- Simplifies initial implementation (interrupts come in Milestone #14)
- Common pattern (Linux `ndo_start_xmit`, smoltcp)
- API remains stable when adding interrupt-driven I/O

**Why non-blocking receive?**
- Protocol stacks poll in loops (e.g., `loop { if let Some(f) = receive() { ... } }`)
- Matches smoltcp's token-based API expectations
- No thread blocking in bare-metal single-core environment

**Why single-frame API (no queues)?**
- Implementations use hardware ring buffers internally (GENET)
- Trait stays simple and focused
- Protocol stacks manage their own packet buffers

**Why frame size validation (60-1514 bytes)?**
- Enforces IEEE 802.3 Ethernet constraints at trait level
- Prevents invalid frames from reaching hardware
- Source: IEEE 802.3 Ethernet standard

---

## Ethernet II Frames

Module: `src/net/ethernet.rs`

Ethernet II is the standard frame format for modern Ethernet networks. It consists of a 14-byte header followed by payload data.

### Frame Structure

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
┌───────────────────────────────────────────────────────────────┐
│                    Destination MAC Address                    │
│                         (6 bytes)                             │
├───────────────────────────────────────────────────────────────┤
│                      Source MAC Address                       │
│                         (6 bytes)                             │
├───────────────────────────────┬───────────────────────────────┤
│        EtherType (2)          │          Payload ...          │
├───────────────────────────────┴───────────────────────────────┤
│                                                               │
│                       Payload Data                            │
│                    (46-1500 bytes)                            │
│                                                               │
├───────────────────────────────────────────────────────────────┤
│                         CRC (4 bytes)                         │
│                    (Calculated by hardware)                   │
└───────────────────────────────────────────────────────────────┘

Total: 64-1518 bytes (excluding preamble/SFD)
```

**Field Descriptions**:

- **Destination MAC**: 48-bit address of the recipient (or broadcast FF:FF:FF:FF:FF:FF)
- **Source MAC**: 48-bit address of the sender
- **EtherType**: 16-bit protocol identifier (big-endian)
  - `0x0800` = IPv4
  - `0x0806` = ARP
  - `0x86DD` = IPv6
- **Payload**: Protocol data (46-1500 bytes, padded if needed)
- **CRC**: Frame check sequence (calculated and verified by hardware)

### MAC Address Representation

#### `MacAddress` Type

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MacAddress(pub [u8; 6]);
```

A MAC address is a 48-bit (6-byte) unique hardware identifier. It's displayed in colon-separated hexadecimal format: `B8:27:EB:12:34:56`.

#### Special Addresses

```rust
// Broadcast - send to all devices on the network
let broadcast = MacAddress::broadcast(); // FF:FF:FF:FF:FF:FF

// Zero address - used in ARP requests for unknown MAC
let zero = MacAddress::zero(); // 00:00:00:00:00:00

// Check address type
if mac.is_broadcast() { /* ... */ }
if mac.is_multicast() { /* Bit 0 of first byte is 1 */ }
if mac.is_unicast() { /* Not multicast */ }
```

#### Parsing and Display

```rust
// Parse from string
let mac: MacAddress = "B8:27:EB:12:34:56".parse().unwrap();

// Display
println!("MAC: {}", mac); // Prints: "B8:27:EB:12:34:56"

// Access bytes
let bytes = mac.as_bytes(); // &[u8; 6]
```

### Ethernet Frame Handling

#### `EthernetFrame` Type

```rust
pub struct EthernetFrame<'a> {
    pub dest_mac: MacAddress,
    pub src_mac: MacAddress,
    pub ethertype: u16,        // Big-endian
    pub payload: &'a [u8],
}
```

The frame uses a lifetime `'a` because the payload is a borrowed slice - it doesn't own the data, just references it.

#### Frame Constants

```rust
const HEADER_SIZE: usize = 14;         // Dest MAC + Src MAC + EtherType
const MIN_PAYLOAD_SIZE: usize = 46;    // Minimum payload (padded if needed)
const MAX_PAYLOAD_SIZE: usize = 1500;  // MTU (Maximum Transmission Unit)
const MIN_FRAME_SIZE: usize = 60;      // 14 + 46 (excluding CRC)
const MAX_FRAME_SIZE: usize = 1514;    // 14 + 1500 (excluding CRC)
```

Source: IEEE 802.3 Ethernet standard

#### Creating a Frame

```rust
use daedalus::net::ethernet::*;

// Create frame
let frame = EthernetFrame::new(
    MacAddress::broadcast(),                      // Destination
    MacAddress::new([0xB8, 0x27, 0xEB, 1, 2, 3]), // Source
    ETHERTYPE_ARP,                                 // Protocol
    &payload_data,                                 // Data
);

// Serialize to buffer
let mut buffer = [0u8; 1518];
let size = frame.write_to(&mut buffer).unwrap();

// Now buffer[0..size] contains the raw frame
```

#### Parsing a Frame

```rust
// Receive raw frame from hardware
let raw_frame: &[u8] = receive_from_hardware();

// Parse
if let Some(frame) = EthernetFrame::parse(raw_frame) {
    println!("From: {}", frame.src_mac);
    println!("To: {}", frame.dest_mac);

    match frame.ethertype {
        ETHERTYPE_ARP => handle_arp(frame.payload),
        ETHERTYPE_IPV4 => handle_ipv4(frame.payload),
        _ => println!("Unknown protocol: {:#06X}", frame.ethertype),
    }
}
```

#### Byte Order Handling

**CRITICAL**: Network protocols use big-endian byte order, but ARM is little-endian.

```rust
// WRONG - sends in little-endian
let ethertype = 0x0806u16;
buffer[12] = (ethertype & 0xFF) as u8;        // 0x06
buffer[13] = ((ethertype >> 8) & 0xFF) as u8; // 0x08

// CORRECT - sends in big-endian
let ethertype_bytes = ethertype.to_be_bytes(); // [0x08, 0x06]
buffer[12..14].copy_from_slice(&ethertype_bytes);
```

The `EthernetFrame` implementation handles this automatically:

```rust
// Write ethertype (big-endian)
let ethertype_bytes = self.ethertype.to_be_bytes();
buffer[12..14].copy_from_slice(&ethertype_bytes);

// Parse ethertype (big-endian)
let ethertype = u16::from_be_bytes([buffer[12], buffer[13]]);
```

### EtherType Values

```rust
pub const ETHERTYPE_IPV4: u16 = 0x0800;  // Internet Protocol v4
pub const ETHERTYPE_ARP: u16 = 0x0806;   // Address Resolution Protocol
pub const ETHERTYPE_IPV6: u16 = 0x86DD;  // Internet Protocol v6
```

Source: IEEE 802 Numbers - <https://www.iana.org/assignments/ieee-802-numbers/>

---

## ARP (Address Resolution Protocol)

Module: `src/net/arp.rs`

ARP is used to map IP addresses to MAC addresses on a local network. When you want to send a packet to IP `192.168.1.1`, ARP determines the MAC address of that device.

### ARP Packet Structure

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
┌───────────────────────────────┬───────────────────────────────┐
│      Hardware Type (2)        │      Protocol Type (2)        │
│         (0x0001)              │         (0x0800)              │
├───────────────┬───────────────┼───────────────────────────────┤
│  HW Addr Len  │ Proto Addr Len│        Operation (2)          │
│      (1)      │      (1)      │     (1=Req, 2=Reply)          │
├───────────────┴───────────────┴───────────────────────────────┤
│                  Sender Hardware Address                      │
│                      (6 bytes - MAC)                          │
├───────────────────────────────────────────────────────────────┤
│            Sender Protocol Address (4 bytes - IPv4)           │
├───────────────────────────────────────────────────────────────┤
│                  Target Hardware Address                      │
│                      (6 bytes - MAC)                          │
├───────────────────────────────────────────────────────────────┤
│            Target Protocol Address (4 bytes - IPv4)           │
└───────────────────────────────────────────────────────────────┘

Total: 28 bytes (for Ethernet/IPv4)
```

**Note**: This packet is carried as the payload of an Ethernet frame with EtherType 0x0806.

### ARP Operation Types

```rust
#[repr(u16)]
pub enum ArpOperation {
    Request = 1,  // "Who has IP X? Tell IP Y"
    Reply = 2,    // "IP X is at MAC Z"
}
```

### ARP Request Example

**Scenario**: Device A (192.168.1.100) wants to communicate with device B (192.168.1.1) but doesn't know B's MAC address.

**Ethernet Frame**:
```
Dest MAC: FF:FF:FF:FF:FF:FF (broadcast - everyone receives it)
Src MAC:  B8:27:EB:12:34:56 (Device A)
EtherType: 0x0806 (ARP)
```

**ARP Packet**:
```
Hardware Type: 0x0001 (Ethernet)
Protocol Type: 0x0800 (IPv4)
HW Addr Len: 6
Proto Addr Len: 4
Operation: 1 (Request)
Sender MAC: B8:27:EB:12:34:56 (Device A)
Sender IP: 192.168.1.100
Target MAC: 00:00:00:00:00:00 (unknown - that's what we're asking)
Target IP: 192.168.1.1 (who we're looking for)
```

**Human Translation**: "This is B8:27:EB:12:34:56 at 192.168.1.100. Who has 192.168.1.1? Please tell me!"

### ARP Reply Example

**Response**: Device B (192.168.1.1) sends a unicast reply to Device A.

**Ethernet Frame**:
```
Dest MAC: B8:27:EB:12:34:56 (Device A - unicast, not broadcast)
Src MAC:  AA:BB:CC:DD:EE:FF (Device B)
EtherType: 0x0806 (ARP)
```

**ARP Packet**:
```
Hardware Type: 0x0001 (Ethernet)
Protocol Type: 0x0800 (IPv4)
HW Addr Len: 6
Proto Addr Len: 4
Operation: 2 (Reply)
Sender MAC: AA:BB:CC:DD:EE:FF (Device B - the answer!)
Sender IP: 192.168.1.1
Target MAC: B8:27:EB:12:34:56 (Device A)
Target IP: 192.168.1.100
```

**Human Translation**: "I'm AA:BB:CC:DD:EE:FF at 192.168.1.1. This is for you, B8:27:EB:12:34:56!"

### Using the ARP API

#### Creating an ARP Request

```rust
use daedalus::net::arp::*;
use daedalus::net::ethernet::*;

// We are 192.168.1.100, asking for 192.168.1.1
let our_mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
let our_ip = [192, 168, 1, 100];
let target_ip = [192, 168, 1, 1];

// Create ARP request
let arp_request = ArpPacket::request(our_mac, our_ip, target_ip);

// Serialize to buffer
let mut arp_buffer = [0u8; 28];
arp_request.write_to(&mut arp_buffer).unwrap();

// Wrap in Ethernet frame (broadcast)
let eth_frame = EthernetFrame::new(
    MacAddress::broadcast(),  // Send to everyone
    our_mac,
    ETHERTYPE_ARP,
    &arp_buffer,
);

// Serialize and send
let mut frame_buffer = [0u8; 64];
let size = eth_frame.write_to(&mut frame_buffer).unwrap();
send_frame(&frame_buffer[..size]);
```

#### Handling an ARP Request

```rust
// Receive Ethernet frame
let eth_frame = EthernetFrame::parse(received_data)?;

// Check if it's ARP
if eth_frame.ethertype == ETHERTYPE_ARP {
    // Parse ARP packet
    if let Some(arp) = ArpPacket::parse(eth_frame.payload) {
        match arp.operation {
            ArpOperation::Request => {
                // Is this request for our IP?
                if arp.target_ip == our_ip {
                    // Send ARP reply
                    let reply = ArpPacket::reply(
                        our_mac,           // Sender MAC (us)
                        our_ip,            // Sender IP (us)
                        arp.sender_mac,    // Target MAC (them)
                        arp.sender_ip,     // Target IP (them)
                    );

                    send_arp_reply(reply);
                }
            }
            ArpOperation::Reply => {
                // Update ARP cache
                cache.insert(arp.sender_ip, arp.sender_mac);
            }
        }
    }
}
```

#### Creating an ARP Reply

```rust
// Responding to a request
let reply = ArpPacket::reply(
    our_mac,              // Who we are
    our_ip,
    requesters_mac,       // Who asked
    requesters_ip,
);

// Serialize
let mut buffer = [0u8; 28];
reply.write_to(&mut buffer).unwrap();

// Wrap in Ethernet frame (unicast to requester)
let frame = EthernetFrame::new(
    requesters_mac,  // Direct reply, not broadcast
    our_mac,
    ETHERTYPE_ARP,
    &buffer,
);
```

### ARP Packet Display

The `ArpPacket` type implements `Display` for debugging:

```rust
println!("{}", arp_packet);

// Output:
// ARP Request - Who has 192.168.1.1? Tell 192.168.1.100 (B8:27:EB:12:34:56)
// ARP Reply - Who has 192.168.1.100? Tell 192.168.1.1 (AA:BB:CC:DD:EE:FF)
```

---

## Network Byte Order

**CRITICAL CONCEPT**: Network protocols use big-endian byte order (most significant byte first), but ARM processors are little-endian (least significant byte first).

### The Problem

```rust
// A 16-bit value: 0x0806 (ARP EtherType)
//
// In memory on ARM (little-endian):
//   buffer[0] = 0x06  (low byte)
//   buffer[1] = 0x08  (high byte)
//
// On the network (big-endian):
//   byte 0 = 0x08 (high byte)
//   byte 1 = 0x06 (low byte)
```

### The Solution

Rust provides conversion functions:

```rust
// Convert to big-endian (for sending)
let value: u16 = 0x0806;
let bytes = value.to_be_bytes();  // [0x08, 0x06] - correct for network

// Convert from big-endian (for receiving)
let bytes: [u8; 2] = [0x08, 0x06];
let value = u16::from_be_bytes(bytes);  // 0x0806
```

### In Practice

```rust
// Writing a 16-bit field to a network packet
let ethertype: u16 = 0x0806;
buffer[12..14].copy_from_slice(&ethertype.to_be_bytes());

// Reading a 16-bit field from a network packet
let ethertype = u16::from_be_bytes([buffer[12], buffer[13]]);
```

**Rule of Thumb**: Any multi-byte field in a network protocol must use `.to_be_bytes()` when writing and `.from_be_bytes()` when reading.

---

## Frame Processing Pipeline

### Transmission (TX) Path

```
Application
    │
    ├─► Create protocol packet (e.g., ARP)
    │   └─► Serialize to buffer
    │
    ├─► Wrap in Ethernet frame
    │   ├─► Set destination MAC
    │   ├─► Set source MAC (our MAC)
    │   ├─► Set EtherType
    │   └─► Add payload
    │
    ├─► Serialize frame to buffer
    │   └─► Handle byte order conversion
    │
    └─► Send to hardware
        └─► GENET TX (future implementation)
```

### Reception (RX) Path

```
Hardware
    │
    ├─► GENET RX (future implementation)
    │
    ├─► Parse Ethernet frame
    │   ├─► Extract dest MAC
    │   ├─► Extract src MAC
    │   ├─► Extract EtherType
    │   └─► Extract payload
    │
    ├─► Filter by destination
    │   ├─► Is it for us? (our MAC or broadcast)
    │   └─► Ignore if not for us
    │
    ├─► Dispatch by EtherType
    │   ├─► 0x0806 → ARP handler
    │   ├─► 0x0800 → IPv4 handler (future)
    │   └─► Other → Log and drop
    │
    └─► Protocol handler
        └─► Parse protocol packet
            └─► Process and respond if needed
```

---

## Testing Strategy

### Unit Tests

The network protocol modules have comprehensive unit tests (30 tests total):

**MAC Address Tests** (12 tests):
- Construction and constants
- Broadcast/multicast detection
- String parsing and display
- Validation

**Ethernet Frame Tests** (6 tests):
- Frame parsing from raw bytes
- Frame serialization to bytes
- Roundtrip (serialize → parse)
- Buffer size validation
- EtherType constants

**ARP Packet Tests** (12 tests):
- Request/reply creation
- Packet parsing
- Packet serialization
- Roundtrip
- Invalid packet handling
- Display formatting

### Running Tests

```bash
# Run all tests
cargo test

# Run only network tests
cargo test --lib net

# Run specific test
cargo test test_arp_request_creation
```

All tests run in QEMU without requiring real hardware.

### Test Coverage

**What's Tested**:
- ✅ Data structure creation and initialization
- ✅ Parsing from raw bytes
- ✅ Serialization to raw bytes
- ✅ Byte order conversion
- ✅ Validation and error handling
- ✅ Display/formatting

**What's Not Tested** (requires hardware):
- ❌ Actual frame transmission
- ❌ Actual frame reception
- ❌ ARP cache management
- ❌ Network timeouts and retries

---

## Future Extensions

### ARP Cache

An ARP cache stores IP-to-MAC mappings to avoid repeated ARP requests:

```rust
struct ArpEntry {
    ip: [u8; 4],
    mac: MacAddress,
    timestamp: u64,  // For expiration (typical: 60 seconds)
}

struct ArpCache {
    entries: Vec<ArpEntry>,
}

impl ArpCache {
    fn lookup(&self, ip: [u8; 4]) -> Option<MacAddress> { /* ... */ }
    fn insert(&mut self, ip: [u8; 4], mac: MacAddress) { /* ... */ }
    fn remove_expired(&mut self, current_time: u64) { /* ... */ }
}
```

### Gratuitous ARP

A gratuitous ARP is an ARP request for your own IP address. It's used to:
- Announce your presence on the network
- Update other devices' ARP caches
- Detect IP address conflicts

```rust
// Send gratuitous ARP (announce our presence)
let gratuitous = ArpPacket::request(our_mac, our_ip, our_ip);
send_broadcast(gratuitous);
```

### IPv4 Integration

When IPv4 is implemented, ARP will be used automatically:

```rust
// Application wants to send IP packet to 192.168.1.1
fn send_ip_packet(dest_ip: [u8; 4], payload: &[u8]) {
    // Look up MAC address
    let dest_mac = match arp_cache.lookup(dest_ip) {
        Some(mac) => mac,
        None => {
            // Send ARP request and wait for reply
            send_arp_request(dest_ip);
            wait_for_arp_reply(dest_ip, timeout)
        }
    };

    // Now we can send the packet
    send_ethernet_frame(dest_mac, ETHERTYPE_IPV4, payload);
}
```

### Proxy ARP

A device can respond to ARP requests on behalf of another device (used in routing):

```rust
// If we're a router, we might answer ARP for devices on other networks
if arp.operation == ArpOperation::Request {
    if should_proxy_for(arp.target_ip) {
        let reply = ArpPacket::reply(
            our_mac,           // We answer with our MAC
            arp.target_ip,     // But claim to be the target IP
            arp.sender_mac,
            arp.sender_ip,
        );
        send_reply(reply);
    }
}
```

---

## Common Patterns

### Pattern 1: Receiving and Dispatching

```rust
fn handle_received_frame(raw_data: &[u8]) {
    // Parse Ethernet frame
    let frame = match EthernetFrame::parse(raw_data) {
        Some(f) => f,
        None => {
            println!("Invalid Ethernet frame");
            return;
        }
    };

    // Filter by destination
    if !frame.dest_mac.is_broadcast() && frame.dest_mac != OUR_MAC {
        // Not for us
        return;
    }

    // Dispatch by protocol
    match frame.ethertype {
        ETHERTYPE_ARP => handle_arp(&frame),
        ETHERTYPE_IPV4 => handle_ipv4(&frame),
        _ => println!("Unknown EtherType: {:#06X}", frame.ethertype),
    }
}
```

### Pattern 2: Sending a Protocol Message

```rust
fn send_arp_request(target_ip: [u8; 4]) -> Result<(), Error> {
    // Create ARP packet
    let arp = ArpPacket::request(OUR_MAC, OUR_IP, target_ip);

    // Serialize ARP
    let mut arp_buffer = [0u8; 28];
    arp.write_to(&mut arp_buffer)?;

    // Wrap in Ethernet frame
    let frame = EthernetFrame::new(
        MacAddress::broadcast(),
        OUR_MAC,
        ETHERTYPE_ARP,
        &arp_buffer,
    );

    // Serialize frame
    let mut frame_buffer = [0u8; 64];
    let size = frame.write_to(&mut frame_buffer)?;

    // Send to hardware
    genet.transmit(&frame_buffer[..size])
}
```

### Pattern 3: Processing ARP Requests

```rust
fn handle_arp(frame: &EthernetFrame) {
    // Parse ARP packet
    let arp = match ArpPacket::parse(frame.payload) {
        Some(a) => a,
        None => return,
    };

    match arp.operation {
        ArpOperation::Request => {
            // Is this for us?
            if arp.target_ip == OUR_IP {
                // Send reply
                let reply = ArpPacket::reply(
                    OUR_MAC,
                    OUR_IP,
                    arp.sender_mac,
                    arp.sender_ip,
                );
                send_arp_reply(reply);
            }
        }
        ArpOperation::Reply => {
            // Update cache
            println!("Learned: {} is at {}",
                     format_ip(&arp.sender_ip),
                     arp.sender_mac);
            arp_cache.insert(arp.sender_ip, arp.sender_mac);
        }
    }
}
```

---

## References

### RFCs and Standards

- **RFC 826**: ARP - Address Resolution Protocol
  - <https://www.rfc-editor.org/rfc/rfc826>

- **IEEE 802.3**: Ethernet Standards
  - Frame format, MAC addressing, physical layer

- **IEEE 802 Numbers**: EtherType Values
  - <https://www.iana.org/assignments/ieee-802-numbers/>

### Implementation References

- **smoltcp**: Future TCP/IP stack for embedded systems
  - <https://github.com/smoltcp-rs/smoltcp>
  - Excellent reference for no_std network implementations

- **Linux Kernel**: Networking stack
  - `net/ethernet/eth.c` - Ethernet handling
  - `net/ipv4/arp.c` - ARP implementation

---

## Debugging Tips

### Viewing Raw Bytes

```rust
fn dump_frame(data: &[u8]) {
    println!("Frame dump ({} bytes):", data.len());
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("{:04X}: ", i * 16);
        for byte in chunk {
            print!("{:02X} ", byte);
        }
        println!();
    }
}
```

### Verifying Byte Order

```rust
// Check if EtherType is correct
let raw = [0x08, 0x06]; // Network bytes
let value = u16::from_be_bytes(raw);
assert_eq!(value, 0x0806); // ARP

// If this fails, byte order is wrong
let wrong = u16::from_le_bytes(raw); // DON'T DO THIS
assert_eq!(wrong, 0x0608); // Backwards!
```

### Packet Capture Simulation

```rust
// Save frames to analyze with Wireshark later
fn save_pcap(frames: &[Vec<u8>], filename: &str) {
    // Write PCAP file format
    // Can load in Wireshark for detailed analysis
}
```

### Common Issues

**Issue**: Frames are being ignored
- **Check**: Is dest MAC correct? (our MAC or broadcast)
- **Check**: Is EtherType in network byte order?

**Issue**: ARP replies not working
- **Check**: Are sender/target fields swapped correctly?
- **Check**: Is the Ethernet frame using unicast dest MAC?

**Issue**: Byte order errors
- **Check**: Using `.to_be_bytes()` and `.from_be_bytes()`?
- **Check**: Not mixing up little-endian and big-endian?

---

## Next Steps

### Integration with Network Device

Once frame TX/RX is implemented (Milestone #13):

```rust
use daedalus::drivers::netdev::NetworkDevice;
use daedalus::drivers::genet::GenetController;

// Initialize networking (works with any NetworkDevice implementation)
let mut netdev = GenetController::new();
if netdev.is_present() {
    netdev.init()?;

    // Send frames
    fn send_frame<T: NetworkDevice>(netdev: &mut T, data: &[u8]) -> Result<(), NetworkError> {
        netdev.transmit(data)
    }

    // Receive frames (polling loop)
    loop {
        if let Some(frame_data) = netdev.receive() {
            handle_received_frame(frame_data);
        }
    }
}
```

### TCP/IP Stack (smoltcp)

Future integration with smoltcp will provide:
- IPv4 and IPv6
- TCP and UDP
- ICMP (ping)
- DHCP client
- DNS client
- HTTP client/server

The Ethernet and ARP modules provide the foundation for smoltcp's Device trait.

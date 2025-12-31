# GENET v5 Ethernet Controller

**Hardware**: Broadcom GENET v5 (Gigabit Ethernet MAC)
**SoC**: BCM2711 (Raspberry Pi 4)
**Driver**: `src/drivers/net/ethernet/broadcom/genet.rs`
**Status**: Hardware detection and PHY management implemented

---

## Overview

The GENET (Gigabit Ethernet) controller is an integrated MAC (Media Access Control) layer device in the BCM2711 SoC. It handles Ethernet frame transmission and reception, communicates with the external PHY chip via MDIO, and provides DMA engines for efficient packet transfer.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    DaedalusOS Driver                     │
│                  (src/drivers/genet.rs)                  │
└───────────────────────────┬──────────────────────────────┘
                            │
            ┌───────────────┴───────────────┐
            │                               │
            ▼                               ▼
┌───────────────────────┐       ┌───────────────────────┐
│   GENET Controller    │       │    MDIO/MDC Bus       │
│   (MAC Layer)         │       │   (Management)        │
│                       │       │                       │
│ • UMAC (UniMAC)       │       │ • PHY Register Access │
│ • RX/TX Buffers       │◄──────┤ • Clause 22 Protocol  │
│ • DMA Engines         │       │ • 1 MHz Clock         │
│ • Interrupt Control   │       │                       │
│ • Statistics Counters │       │                       │
└───────────┬───────────┘       └───────────┬───────────┘
            │                               │
            │ Frame Data                    │ Management
            │                               │
            └───────────────┬───────────────┘
                            │
                            ▼
                ┌───────────────────────┐
                │   BCM54213PE PHY      │
                │   (Physical Layer)    │
                │                       │
                │ • Auto-negotiation    │
                │ • Link Detection      │
                │ • 10/100/1000 Mbps    │
                │ • MII Registers       │
                └───────────┬───────────┘
                            │
                            ▼
                    RJ45 Ethernet Port
```

### Understanding "MAC" - Two Different Meanings

**Terminology Confusion**: "MAC" has two completely different meanings in networking:

| Term | Meaning | What It Is |
|------|---------|------------|
| **MAC Address** | Media Access Control Address | 48-bit hardware address (e.g., `B8:27:EB:12:34:56`) |
| **MAC Chip** | Media Access Controller | Physical chip that implements Layer 2 ethernet protocol |

When this document refers to "GENET MAC" or "MAC controller," it means the **controller chip**, not the address!

### PHY vs MAC: The Two-Chip Architecture

Modern ethernet requires **two separate chips** with different responsibilities:

#### MAC Chip (GENET) - Inside BCM2711 SoC
- **Location**: Integrated into the CPU die
- **Layer**: OSI Layer 2 (Data Link)
- **Works with**: Digital signals (bits)
- **Responsibilities**:
  - Build/parse ethernet frames (14-byte header)
  - Add/check CRC-32 (only CRC - IP/TCP checksums are software!)
  - MAC address filtering (accept frames for our address)
  - DMA to/from system memory
  - Send/receive bits to/from PHY via RGMII bus

#### PHY Chip (BCM54213PE) - External on Pi 4 Board
- **Location**: Separate chip on the board (not in SoC)
- **Layer**: OSI Layer 1 (Physical)
- **Works with**: Analog signals (voltages on copper wire)
- **Responsibilities**:
  - Convert digital bits ↔ electrical signals
  - Auto-negotiation (determine speed/duplex with partner)
  - Link detection (is cable plugged in?)
  - Signal encoding (1000BASE-T, 100BASE-TX, etc.)
  - Cable equalization, echo cancellation

#### How They Communicate

**Data Path (RGMII)**: 12-wire parallel bus transfers packet bits
- 4-bit TX data, 4-bit RX data, clocks, control signals
- 125 MHz for Gigabit (1000 Mbps)
- Carries actual ethernet frame data

**Management Path (MDIO)**: 2-wire serial bus for PHY configuration
- MDC (clock), MDIO (data)
- ~1 MHz, slow but sufficient for configuration
- Read PHY ID, configure speed, check link status

#### What the Driver Actually Does

**95% of driver code** = MAC chip (GENET)
- Configure DMA rings
- Build ethernet frames (header + payload)
- Enable TX/RX
- Handle interrupts
- Manage buffers

**5% of driver code** = PHY chip (BCM54213PE)
- Reset PHY
- Enable auto-negotiation
- Read link status

The PHY mostly "just works" once configured. The MAC is where driver complexity lives.

### Key Features

- **MAC Layer**: Handles frame encapsulation, CRC, and media access control
- **MDIO Controller**: Manages communication with the PHY chip
- **DMA Engines**: Separate RX and TX DMA for efficient packet transfer (not yet implemented)
- **Hardware Filtering**: Can filter packets by MAC address (not yet implemented)
- **Statistics**: Hardware counters for packets, bytes, errors (not yet implemented)
- **Interrupts**: RX/TX completion, link changes, errors (not yet implemented)

---

## Memory Map

### Base Address

| Address Type | Value | Note |
|--------------|-------|------|
| **Bus Address** | `0x7D580000` | As seen in device tree |
| **ARM Physical** | `0xFD580000` | What the CPU uses (bus + 0x8000_0000) |
| **Size** | `0x10000` (64 KB) | Register space |

**CRITICAL**: Always use `0xFD580000` as the base address. The device tree uses bus addresses, which differ from ARM physical addresses by a fixed offset.

Source: BCM2711 device tree (`bcm2711.dtsi`)

### Register Block Offsets

All offsets are from `GENET_BASE` (0xFD580000):

| Block | Offset | Size | Description |
|-------|--------|------|-------------|
| **SYS** | `0x0000` | 64 B | System control registers |
| **GR_BRIDGE** | `0x0040` | 64 B | GR bridge registers |
| **EXT** | `0x0080` | 384 B | Extension block |
| **INTRL2_0** | `0x0200` | 64 B | Interrupt controller 0 |
| **INTRL2_1** | `0x0240` | 64 B | Interrupt controller 1 |
| **RBUF** | `0x0300` | 768 B | RX buffer control |
| **TBUF** | `0x0600` | 512 B | TX buffer control |
| **UMAC** | `0x0800` | 3588 B | UniMAC (the actual MAC) |
| **RDMA** | `0x2000` | 8192 B | RX DMA engine |
| **TDMA** | `0x4000` | 8192 B | TX DMA engine |
| **HFB** | `0x8000` | 32768 B | Hardware filter block |

Source: Linux kernel driver (`bcmgenet.h`)

---

## Register Reference

### System Registers (SYS_OFF = 0x0000)

#### SYS_REV_CTRL (Offset 0x0000)

System revision control register. Contains version information.

**Format**:
```
Bits [31:28]: Reserved
Bits [27:24]: Major version (4 bits)
Bits [23:20]: Reserved
Bits [19:16]: Minor version (4 bits)
Bits [15:0]:  Reserved
```

**⚠️ CRITICAL VERSION QUIRK**: The BCM2711 GENET hardware reports major version **6** (not 5), which corresponds to the GENET v5 IP block. This naming inconsistency exists across all drivers:
- **Hardware register value**: `0x06000000` (major=6, minor=0)
- **IP block name**: GENET v5
- **Linux enum**: `GENET_V5 = 5`
- **U-Boot validation**: Only accepts major version 6

**Examples**:
- `0x06000000` = GENET hardware v6.0 (GENET v5 IP block) ← **Raspberry Pi 4**
- `0x05020000` = GENET hardware v5.2 (GENET v4 IP block)

**Usage**: The `is_present()` function checks that bits [27:24] == 6 for BCM2711.

**Sources**:
- U-Boot: `major = (reg >> 24) & 0x0f; if (major != 6) reject;`
- Linux: Maps hardware version 6 → `GENET_V5` enum
- EDK2: `SYS_REV_MAJOR = BIT27|BIT26|BIT25|BIT24`

---

### UMAC Registers (UMAC_OFF = 0x0800)

The UMAC (Unified MAC) is the core MAC layer implementation within GENET.

#### UMAC_CMD (Offset 0x0808)

Command register. Controls MAC enable, reset, and operating modes.

**Key Bits**:
- Bit 0: `TX_EN` - Enable transmit
- Bit 1: `RX_EN` - Enable receive
- Bit 13: `SW_RESET` - Software reset (self-clearing)

**Usage**:
```rust
// Enable TX and RX
self.write_reg(UMAC_CMD, CMD_TX_EN | CMD_RX_EN);

// Reset UMAC
self.write_reg(UMAC_CMD, CMD_SW_RESET);
// Wait for reset to complete (bit clears automatically)
```

#### UMAC_MAC0 (Offset 0x080C)

MAC address bytes 0-3 (network byte order).

**Format**:
```
Bits [31:24]: MAC byte 0
Bits [23:16]: MAC byte 1
Bits [15:8]:  MAC byte 2
Bits [7:0]:   MAC byte 3
```

#### UMAC_MAC1 (Offset 0x0810)

MAC address bytes 4-5 (network byte order).

**Format**:
```
Bits [31:16]: Reserved
Bits [15:8]:  MAC byte 4
Bits [7:0]:   MAC byte 5
```

**Usage**:
```rust
// Set MAC address B8:27:EB:12:34:56
let mac0 = (0xB8 << 24) | (0x27 << 16) | (0xEB << 8) | 0x12;
let mac1 = (0x34 << 8) | 0x56;
self.write_reg(UMAC_MAC0, mac0);
self.write_reg(UMAC_MAC1, mac1);
```

#### UMAC_MODE (Offset 0x084C)

Mode register. Controls speed (10/100/1000 Mbps) and duplex.

**⚠️ HARDWARE QUIRK**: This register is write-only. Reading returns garbage. Must track state in software.

**Key Bits**:
- Bits [1:0]: Speed selection
  - `00` = 10 Mbps
  - `01` = 100 Mbps
  - `10` = 1000 Mbps
- Bit 4: Full duplex enable

#### UMAC_MDIO_CMD (Offset 0x0E14)

MDIO command and data register. Used to read/write PHY registers.

**Format**:
```
Bit 29:       MDIO_START_BUSY - Start operation / operation in progress
Bit 28:       MDIO_READ_FAIL - Read failed
Bits [27:26]: Operation - 10 = read, 01 = write
Bits [25:21]: PHY address (5 bits)
Bits [20:16]: Register address (5 bits)
Bits [15:0]:  Data (read or write)
```

**Read Sequence**:
1. Write: `MDIO_START_BUSY | MDIO_RD | (phy_addr << 21) | (reg_addr << 16)`
2. Poll bit 29 until clear (timeout ~1ms)
3. Check bit 28 (MDIO_READ_FAIL)
4. Read bits [15:0] for data

**Write Sequence**:
1. Write: `MDIO_START_BUSY | MDIO_WR | (phy_addr << 21) | (reg_addr << 16) | data`
2. Poll bit 29 until clear (timeout ~1ms)

**See**: MDIO Protocol section below for details.

---

## MDIO Protocol

MDIO (Management Data Input/Output) is the bus used to communicate with the PHY chip. It's a simple serial protocol with two signals:

- **MDC**: Management Data Clock (~1 MHz)
- **MDIO**: Management Data (bidirectional)

### Clause 22 Protocol

The GENET controller implements IEEE 802.3 Clause 22 MDIO protocol:

1. **Preamble**: 32 bits of `1`
2. **Start**: `01`
3. **Opcode**: `10` (read) or `01` (write)
4. **PHY Address**: 5 bits
5. **Register Address**: 5 bits
6. **Turnaround**: 2 bits
7. **Data**: 16 bits

**Timing**: Each bit takes one MDC clock cycle. The GENET controller handles the protocol automatically - we just write to `UMAC_MDIO_CMD` and poll for completion.

### MDIO Operations

#### Reading a PHY Register

```rust
pub fn mdio_read(&self, phy_addr: u8, reg_addr: u8) -> Option<u16> {
    // Build command: read operation
    let cmd = MDIO_START_BUSY
        | MDIO_RD
        | ((phy_addr as u32) << 21)
        | ((reg_addr as u32) << 16);

    // Start operation
    self.write_reg(UMAC_MDIO_CMD, cmd);

    // Wait for completion (poll START_BUSY bit)
    for _ in 0..1000 {
        let status = self.read_reg(UMAC_MDIO_CMD);

        if (status & MDIO_START_BUSY) == 0 {
            // Check for read failure
            if (status & MDIO_READ_FAIL) != 0 {
                return None;
            }

            // Return data from bits [15:0]
            return Some((status & 0xFFFF) as u16);
        }

        SystemTimer::delay_us(1);
    }

    None // Timeout
}
```

#### Writing a PHY Register

```rust
pub fn mdio_write(&self, phy_addr: u8, reg_addr: u8, value: u16) -> bool {
    // Build command: write operation with data
    let cmd = MDIO_START_BUSY
        | MDIO_WR
        | ((phy_addr as u32) << 21)
        | ((reg_addr as u32) << 16)
        | (value as u32);

    // Start operation
    self.write_reg(UMAC_MDIO_CMD, cmd);

    // Wait for completion
    for _ in 0..1000 {
        let status = self.read_reg(UMAC_MDIO_CMD);

        if (status & MDIO_START_BUSY) == 0 {
            return true;
        }

        SystemTimer::delay_us(1);
    }

    false // Timeout
}
```

**Timeout**: 1000 iterations × 1 µs = 1 ms maximum wait time.

---

## PHY Management (BCM54213PE)

The BCM54213PE is the external Gigabit Ethernet PHY chip on the Raspberry Pi 4. It handles the physical layer: link detection, auto-negotiation, and signal encoding.

### PHY Constants

| Constant | Value | Source |
|----------|-------|--------|
| **MDIO Address** | `0x01` | Pi 4 device tree |
| **PHY ID** | `0x600D84A2` | Linux kernel driver |
| **PHY ID1 Register** | `0x600D` | Upper 16 bits |
| **PHY ID2 Register** | `0x84A2` | Lower 16 bits |

### MII Register Map (IEEE 802.3)

These are standard registers that all Ethernet PHYs must implement:

| Register | Address | Name | Description |
|----------|---------|------|-------------|
| **BMCR** | `0x00` | Basic Mode Control | Control register |
| **BMSR** | `0x01` | Basic Mode Status | Status register |
| **PHYSID1** | `0x02` | PHY ID 1 | Upper 16 bits of PHY ID |
| **PHYSID2** | `0x03` | PHY ID 2 | Lower 16 bits of PHY ID |
| **ADVERTISE** | `0x04` | Auto-Negotiation Advertisement | Capabilities to advertise |
| **LPA** | `0x05` | Link Partner Ability | Partner's capabilities |
| **CTRL1000** | `0x09` | 1000BASE-T Control | Gigabit control |
| **STAT1000** | `0x0A` | 1000BASE-T Status | Gigabit status |

Source: IEEE 802.3 Clause 22

### BMCR - Basic Mode Control Register (0x00)

Controls PHY operation and initiates actions.

**Key Bits**:
- Bit 15: `RESET` - Software reset (self-clearing)
- Bit 12: `ANENABLE` - Enable auto-negotiation
- Bit 9: `ANRESTART` - Restart auto-negotiation
- Bit 8: `DUPLEX` - Full duplex (if auto-neg disabled)
- Bits [6,13]: Speed selection (if auto-neg disabled)

**Usage**:
```rust
// Reset PHY
self.mdio_write(PHY_ADDR, MII_BMCR, BMCR_RESET);
SystemTimer::delay_ms(10); // Wait for reset

// Enable auto-negotiation
self.mdio_write(PHY_ADDR, MII_BMCR, BMCR_ANENABLE | BMCR_ANRESTART);
```

### BMSR - Basic Mode Status Register (0x01)

Read-only register indicating PHY status and capabilities.

**Key Bits**:
- Bit 5: `ANEGCOMPLETE` - Auto-negotiation complete
- Bit 2: `LSTATUS` - Link status (1 = link up)
- Bit 3: Auto-negotiation capable
- Bits [15:11]: Supported speeds (100BASE-T4, 100BASE-X, 10BASE-T)

**Usage**:
```rust
// Check link status
if let Some(bmsr) = self.mdio_read(PHY_ADDR, MII_BMSR) {
    let link_up = (bmsr & BMSR_LSTATUS) != 0;
    let autoneg_done = (bmsr & BMSR_ANEGCOMPLETE) != 0;
}
```

**⚠️ NOTE**: Some BMSR bits are latching (they stick until read). Reading BMSR twice can give different results. Always read twice to get current state.

### PHY Initialization Sequence

1. **Read PHY ID** to verify presence:
   ```rust
   let id1 = self.mdio_read(PHY_ADDR, MII_PHYSID1)?;
   let id2 = self.mdio_read(PHY_ADDR, MII_PHYSID2)?;
   let phy_id = ((id1 as u32) << 16) | (id2 as u32);
   assert_eq!(phy_id, 0x600D84A2); // BCM54213PE
   ```

2. **Software Reset**:
   ```rust
   self.mdio_write(PHY_ADDR, MII_BMCR, BMCR_RESET);
   SystemTimer::delay_ms(10); // Wait for reset to complete
   ```

3. **Configure Auto-Negotiation** (optional, usually done by firmware):
   ```rust
   // Read current advertisement
   let advertise = self.mdio_read(PHY_ADDR, MII_ADVERTISE)?;
   // Advertise 10/100 capabilities...

   // Enable Gigabit advertisement
   let ctrl1000 = self.mdio_read(PHY_ADDR, MII_CTRL1000)?;
   // Set Gigabit capabilities...
   ```

4. **Start Auto-Negotiation**:
   ```rust
   self.mdio_write(PHY_ADDR, MII_BMCR, BMCR_ANENABLE | BMCR_ANRESTART);
   ```

5. **Wait for Link**:
   ```rust
   for _ in 0..3000 {
       if let Some(bmsr) = self.mdio_read(PHY_ADDR, MII_BMSR) {
           if (bmsr & BMSR_ANEGCOMPLETE) != 0 {
               // Auto-negotiation complete
               break;
           }
       }
       SystemTimer::delay_ms(1);
   }
   ```

6. **Read Link Parameters**:
   ```rust
   let lpa = self.mdio_read(PHY_ADDR, MII_LPA)?;
   let stat1000 = self.mdio_read(PHY_ADDR, MII_STAT1000)?;
   // Determine speed and duplex from partner ability
   ```

---

## Hardware Quirks and Limitations

### 1. UMAC_MODE is Write-Only

**Problem**: Reading `UMAC_MODE` register returns garbage, not the written value.

**Impact**: Cannot verify speed/duplex settings by reading back.

**Workaround**: Track the current mode in software (in the `GenetController` struct).

**Source**: U-Boot driver comments, community reports

### 2. PHY Link Change Interrupts Don't Work

**Problem**: The PHY doesn't generate interrupts on link state changes.

**Impact**: Cannot rely on interrupts for link detection.

**Workaround**: Poll `BMSR` register periodically (e.g., every 1 second) to detect link changes.

**Source**: Linux kernel driver uses polling

### 3. Some Registers Are Write-Once After Reset

**Problem**: Certain configuration registers only accept the first write after a hardware reset.

**Impact**: Must get initialization right the first time.

**Workaround**: Carefully plan initialization sequence. Test thoroughly.

**Source**: Broadcom vendor documentation (not public)

### 4. MDIO Timing is Critical

**Problem**: MDIO operations need proper delays between operations.

**Impact**: Too fast = operation fails. Too slow = waste time.

**Workaround**: Use 1 µs polling intervals with 1 ms timeout (current implementation).

**Source**: IEEE 802.3 timing requirements

### 5. Auto-Negotiation Takes Time

**Problem**: Link auto-negotiation can take 1-3 seconds.

**Impact**: Boot time increases if waiting for link.

**Workaround**:
- Option 1: Don't wait during init, just start negotiation
- Option 2: Wait with timeout and continue even if incomplete
- Option 3: Background polling task

**Source**: IEEE 802.3 specification (auto-negotiation protocol)

---

## QEMU Limitations

**CRITICAL**: QEMU 9.0's `raspi4b` machine does **not** fully emulate GENET.

### Observed Behavior

Reading from GENET registers (`0xFD580000`) in QEMU causes a Data Abort exception. This happens because:

1. QEMU doesn't implement the GENET controller
2. The address is not mapped to any device
3. ARM generates a data abort for unmapped addresses

### Detection

The `is_present()` function safely detects this:

```rust
pub fn is_present(&self) -> bool {
    let version = self.read_reg(SYS_REV_CTRL);
    let major_version = (version >> 16) & 0xFFFF;
    major_version == 0x0005
}
```

In QEMU, this will either:
- Return false (if the read succeeds but returns garbage)
- Cause a data abort exception (current QEMU behavior)

### Workaround

Wrap all GENET access in exception handlers or presence checks:

```rust
if genet.is_present() {
    // Safe to use GENET
    genet.diagnostic();
} else {
    println!("GENET hardware not present (QEMU?)");
}
```

### Testing Strategy

- **Unit Tests**: Test pure functions (parsing, encoding) in QEMU
- **Integration Tests**: Mark as `#[ignore]`, run on real hardware only
- **Interactive Tests**: Use shell commands on real Pi 4

---

## Initialization Flowchart

Complete initialization sequence for GENET + PHY:

```
START
  │
  ├─► Check GENET present (read SYS_REV_CTRL)
  │   ├─► Version != v5 → ERROR: Hardware not found
  │   └─► Version == v5 → Continue
  │
  ├─► Soft reset UMAC (write UMAC_CMD)
  │   └─► Wait 10 µs
  │
  ├─► Detect PHY via MDIO
  │   ├─► Read PHYSID1 (0x02)
  │   ├─► Read PHYSID2 (0x03)
  │   └─► Verify ID == 0x600D84A2
  │
  ├─► Reset PHY
  │   ├─► Write BMCR[15] = 1 (reset)
  │   └─► Wait 10-100 ms
  │
  ├─► Configure Auto-Negotiation
  │   ├─► Write ADVERTISE (10/100 capabilities)
  │   ├─► Write CTRL1000 (1000 capabilities)
  │   └─► Write BMCR (enable auto-neg, restart)
  │
  ├─► Wait for Link (optional)
  │   ├─► Poll BMSR[5] (auto-neg complete)
  │   ├─► Poll BMSR[2] (link status)
  │   └─► Timeout after 3 seconds
  │
  ├─► Read Link Parameters
  │   ├─► Read LPA (partner ability)
  │   ├─► Read STAT1000 (Gigabit status)
  │   └─► Determine speed and duplex
  │
  ├─► Configure UMAC
  │   ├─► Write MAC address (UMAC_MAC0/MAC1)
  │   ├─► Write speed/duplex (UMAC_MODE)
  │   └─► Enable TX/RX (UMAC_CMD)
  │
  └─► READY
```

---

## Diagnostic Output

The `diagnostic()` function performs a comprehensive hardware check. Expected output on real Pi 4:

### With Ethernet Cable Unplugged

```
[DIAG] Ethernet Hardware Diagnostics
[DIAG] ================================
[DIAG] Step 1: GENET Controller Detection
[DIAG]   Reading SYS_REV_CTRL @ 0xFD580000...
[DIAG]   Raw register value: 0x06000000
[PASS]   GENET hardware v6.0 detected (GENET v5 IP block)
[PASS]   Register: 0x06000000

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
[DIAG]       Link status: DOWN
[DIAG]       Auto-negotiation: IN PROGRESS
[DIAG]   Reading BMCR (Basic Mode Control Register)...
[DIAG]     BMCR: 0x1140
[DIAG]       Auto-negotiation: ENABLED

[PASS] ================================
[PASS] Hardware diagnostics complete!
[PASS] GENET hardware v6.0 (GENET v5 IP) and BCM54213PE PHY detected
```

**Note**: Link status shows DOWN when no ethernet cable is plugged in. This is normal - plug in a cable to see "Link status: UP" and "Auto-negotiation: COMPLETE".

### In QEMU

```
[DIAG] Ethernet Hardware Diagnostics
[DIAG] ================================
[DIAG] Step 1: GENET Controller Detection
[DIAG]   Reading SYS_REV_CTRL @ 0xFD580000...
[DIAG]   Raw register value: 0x00000000
[WARN]   Unexpected version: 0.0 (expected 6.x for GENET v5)
[INFO]   Hardware not present (running in QEMU?)
[SKIP] Diagnostics completed (no hardware detected)
```

---

## References

### Official Documentation
- **BCM2711 ARM Peripherals PDF**: <https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf>
  - Section 1.2: Address map (limited GENET coverage)

- **IEEE 802.3**: Ethernet standards
  - Clause 22: MII register definitions
  - Clause 28: Auto-negotiation protocol

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

## Next Steps

### Current Implementation Status

✅ **Implemented**:
- Register definitions and constants
- MMIO read/write functions
- MDIO read/write protocol
- PHY ID detection
- Hardware presence checking
- Comprehensive diagnostics

❌ **Not Yet Implemented**:
- Frame transmission (TX path)
- Frame reception (RX path)
- DMA engine configuration
- Interrupt handling
- MAC address configuration
- Link state monitoring
- Speed/duplex configuration

### Future Milestones

- **Milestone #13**: Frame TX/RX (simple polling mode)
- **Milestone #14**: Interrupt-driven RX
- **Milestone #15**: ARP responder
- **Milestone #16**: TCP/IP stack integration (smoltcp)

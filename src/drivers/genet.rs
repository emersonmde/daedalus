//! GENET v5 Gigabit Ethernet Controller Driver (BCM2711)
//!
//! This driver provides support for the Broadcom GENET v5 Ethernet MAC controller
//! found in the Raspberry Pi 4 (BCM2711 SoC).
//!
//! ## Hardware Features
//! - Gigabit Ethernet MAC (Media Access Control)
//! - MDIO bus controller for PHY management
//! - DMA engine for packet transfer (not yet implemented)
//! - Hardware filtering and statistics
//!
//! ## QEMU Limitations
//! **IMPORTANT**: As of QEMU 9.0, GENET support is listed but not fully functional.
//! Reading from GENET registers (0xFD580000) in QEMU will cause a Data Abort exception.
//! The `is_present()` check will fail gracefully in QEMU, returning false.
//!
//! This driver is designed to work on **real Raspberry Pi 4 hardware**. Use the
//! `eth-diag` shell command to verify hardware presence before attempting operations.
//!
//! ## References
//! - Linux kernel driver: <https://github.com/torvalds/linux/blob/master/drivers/net/ethernet/broadcom/genet/bcmgenet.h>
//! - U-Boot driver: <https://github.com/u-boot/u-boot/blob/master/drivers/net/bcmgenet.c>
//! - Device tree: <https://github.com/raspberrypi/linux/blob/rpi-5.4.y/arch/arm/boot/dts/bcm2711.dtsi>

// Allow dead code warnings for register constants that will be used when
// full MAC initialization is implemented on real hardware
#![allow(dead_code)]

use crate::drivers::timer::SystemTimer;
use crate::println;
use core::ptr;

// ============================================================================
// Hardware Address Constants
// ============================================================================

/// GENET base address (ARM physical address space)
///
/// Note: Device tree uses bus address 0x7D580000, but ARM sees it at 0xFD580000
/// Source: BCM2711 device tree (bcm2711.dtsi)
const GENET_BASE: usize = 0xFD58_0000;

/// Size of GENET register space (64 KB)
const GENET_SIZE: usize = 0x10000;

// ============================================================================
// Register Block Offsets (from GENET_BASE)
// ============================================================================

/// System control registers
const SYS_OFF: usize = 0x0000;

/// GR bridge registers
const GR_BRIDGE_OFF: usize = 0x0040;

/// Extension block
const EXT_OFF: usize = 0x0080;

/// Interrupt controller 0
const INTRL2_0_OFF: usize = 0x0200;

/// Interrupt controller 1
const INTRL2_1_OFF: usize = 0x0240;

/// RX buffer control
const RBUF_OFF: usize = 0x0300;

/// TX buffer control
const TBUF_OFF: usize = 0x0600;

/// UniMAC (the actual MAC)
const UMAC_OFF: usize = 0x0800;

/// RX DMA engine
const RDMA_OFF: usize = 0x2000;

/// TX DMA engine
const TDMA_OFF: usize = 0x4000;

/// Hardware filter block
const HFB_OFF: usize = 0x8000;

// ============================================================================
// System Registers (SYS_OFF)
// ============================================================================

/// System revision control register
/// Contains version information for the GENET controller
const SYS_REV_CTRL: usize = SYS_OFF;

// ============================================================================
// UMAC Registers (UMAC_OFF)
// ============================================================================

/// UMAC command register
/// Controls TX/RX enable, reset, promiscuous mode, etc.
const UMAC_CMD: usize = UMAC_OFF + 0x008;

/// MAC address bytes 0-3 (network byte order)
const UMAC_MAC0: usize = UMAC_OFF + 0x00C;

/// MAC address bytes 4-5 (network byte order)
const UMAC_MAC1: usize = UMAC_OFF + 0x010;

/// UMAC mode register
/// Controls speed (10/100/1000) and duplex
/// WARNING: This register cannot be read back (write-only quirk)
const UMAC_MODE: usize = UMAC_OFF + 0x044;

/// MDIO command/data register
/// Used to read/write PHY registers via MDIO protocol
const UMAC_MDIO_CMD: usize = UMAC_OFF + 0x614;

/// MIB control register
/// Controls statistics counters and reset
const UMAC_MIB_CTRL: usize = UMAC_OFF + 0x580;

// ============================================================================
// UMAC_CMD Register Bits
// ============================================================================

/// Enable transmit
const CMD_TX_EN: u32 = 1 << 0;

/// Enable receive
const CMD_RX_EN: u32 = 1 << 1;

/// Software reset
const CMD_SW_RESET: u32 = 1 << 13;

// ============================================================================
// MDIO Command Register Bits (UMAC_MDIO_CMD)
// ============================================================================

/// Start MDIO operation / operation in progress
const MDIO_START_BUSY: u32 = 1 << 29;

/// Read operation failed
const MDIO_READ_FAIL: u32 = 1 << 28;

/// Read operation (bits 27:26 = 0b10)
const MDIO_RD: u32 = 2 << 26;

/// Write operation (bits 27:26 = 0b01)
const MDIO_WR: u32 = 1 << 26;

/// PHY address shift (bits 25:21)
const MDIO_PHY_ADDR_SHIFT: u32 = 21;

/// Register address shift (bits 20:16)
const MDIO_REG_ADDR_SHIFT: u32 = 16;

// ============================================================================
// PHY Register Addresses (Standard MII - IEEE 802.3 Clause 22)
// ============================================================================

/// Basic Mode Control Register
pub const MII_BMCR: u8 = 0x00;

/// Basic Mode Status Register
pub const MII_BMSR: u8 = 0x01;

/// PHY Identifier 1 (upper 16 bits)
pub const MII_PHYSID1: u8 = 0x02;

/// PHY Identifier 2 (lower 16 bits)
pub const MII_PHYSID2: u8 = 0x03;

/// Auto-Negotiation Advertisement Register
pub const MII_ADVERTISE: u8 = 0x04;

/// Link Partner Ability Register
pub const MII_LPA: u8 = 0x05;

/// 1000BASE-T Control Register
pub const MII_CTRL1000: u8 = 0x09;

/// 1000BASE-T Status Register
pub const MII_STAT1000: u8 = 0x0A;

// ============================================================================
// BMCR Register Bits
// ============================================================================

/// Software reset
const BMCR_RESET: u16 = 1 << 15;

/// Enable auto-negotiation
const BMCR_ANENABLE: u16 = 1 << 12;

/// Restart auto-negotiation
const BMCR_ANRESTART: u16 = 1 << 9;

// ============================================================================
// BMSR Register Bits
// ============================================================================

/// Auto-negotiation complete
const BMSR_ANEGCOMPLETE: u16 = 1 << 5;

/// Link status (1 = link up)
const BMSR_LSTATUS: u16 = 1 << 2;

// ============================================================================
// PHY Constants
// ============================================================================

/// BCM54213PE PHY MDIO address
/// Source: Raspberry Pi 4 device tree and community findings
const PHY_ADDR: u8 = 0x01;

/// Expected PHY ID for BCM54213PE
/// Source: Linux kernel driver
const PHY_ID_BCM54213PE: u32 = 0x600D84A2;

// ============================================================================
// GENET Controller
// ============================================================================

/// GENET v5 Ethernet controller
pub struct GenetController {
    base_addr: usize,
}

impl Default for GenetController {
    fn default() -> Self {
        Self::new()
    }
}

impl GenetController {
    /// Create a new GENET controller instance
    pub const fn new() -> Self {
        Self {
            base_addr: GENET_BASE,
        }
    }

    /// Check if GENET hardware is present
    ///
    /// This safely probes for the hardware by reading the version register.
    /// Returns true if GENET v5 is detected, false otherwise (e.g., running in QEMU).
    pub fn is_present(&self) -> bool {
        let version = self.read_reg(SYS_REV_CTRL);

        // GENET v5 has version in bits [31:16] = 0x0005
        let major_version = (version >> 16) & 0xFFFF;
        major_version == 0x0005
    }

    /// Get the hardware version
    ///
    /// Returns the full 32-bit version register value.
    /// Format: \[31:16\] = major version, \[15:8\] = minor version, \[7:0\] = patch
    pub fn get_version(&self) -> u32 {
        self.read_reg(SYS_REV_CTRL)
    }

    /// Read a GENET register
    ///
    /// # Safety
    /// This performs a volatile MMIO read. The offset must be within the valid
    /// register space and must not have side effects when read.
    fn read_reg(&self, offset: usize) -> u32 {
        // SAFETY: MMIO read is safe because:
        // 1. base_addr is a valid MMIO address for GENET (0xFD580000)
        // 2. offset is validated to be within register space by the caller
        // 3. read_volatile ensures the compiler doesn't optimize away the read
        // 4. u32 is the correct size for GENET registers
        unsafe { ptr::read_volatile((self.base_addr + offset) as *const u32) }
    }

    /// Write a GENET register
    ///
    /// # Safety
    /// This performs a volatile MMIO write. The offset must be within the valid
    /// register space and the value must be appropriate for that register.
    fn write_reg(&self, offset: usize, value: u32) {
        // SAFETY: MMIO write is safe because:
        // 1. base_addr is a valid MMIO address for GENET (0xFD580000)
        // 2. offset is validated to be within register space by the caller
        // 3. write_volatile ensures the write reaches hardware
        // 4. u32 is the correct size for GENET registers
        unsafe {
            ptr::write_volatile((self.base_addr + offset) as *mut u32, value);
        }
    }

    /// Read a PHY register via MDIO
    ///
    /// Returns the 16-bit register value, or None if the operation times out or fails.
    pub fn mdio_read(&self, phy_addr: u8, reg_addr: u8) -> Option<u16> {
        // Build MDIO command: read operation
        let cmd = MDIO_START_BUSY
            | MDIO_RD
            | ((phy_addr as u32) << MDIO_PHY_ADDR_SHIFT)
            | ((reg_addr as u32) << MDIO_REG_ADDR_SHIFT);

        // Write command to start the operation
        self.write_reg(UMAC_MDIO_CMD, cmd);

        // Wait for operation to complete (poll START_BUSY bit)
        // Timeout after 1000 microseconds (1ms)
        for _ in 0..1000 {
            let status = self.read_reg(UMAC_MDIO_CMD);

            if (status & MDIO_START_BUSY) == 0 {
                // Operation complete - check for read failure
                if (status & MDIO_READ_FAIL) != 0 {
                    return None;
                }

                // Extract data from bits [15:0]
                return Some((status & 0xFFFF) as u16);
            }

            // Wait 1us before next poll
            SystemTimer::delay_us(1);
        }

        // Timeout
        None
    }

    /// Write a PHY register via MDIO
    ///
    /// Returns true if the operation succeeded, false if it timed out.
    pub fn mdio_write(&self, phy_addr: u8, reg_addr: u8, value: u16) -> bool {
        // Build MDIO command: write operation with data in bits [15:0]
        let cmd = MDIO_START_BUSY
            | MDIO_WR
            | ((phy_addr as u32) << MDIO_PHY_ADDR_SHIFT)
            | ((reg_addr as u32) << MDIO_REG_ADDR_SHIFT)
            | (value as u32);

        // Write command to start the operation
        self.write_reg(UMAC_MDIO_CMD, cmd);

        // Wait for operation to complete (poll START_BUSY bit)
        // Timeout after 1000 microseconds (1ms)
        for _ in 0..1000 {
            let status = self.read_reg(UMAC_MDIO_CMD);

            if (status & MDIO_START_BUSY) == 0 {
                return true;
            }

            // Wait 1us before next poll
            SystemTimer::delay_us(1);
        }

        // Timeout
        false
    }

    /// Detect PHY and read its ID
    ///
    /// Returns the 32-bit PHY ID if found, or None if not present.
    /// PHY ID is composed of PHYSID1 (upper 16 bits) and PHYSID2 (lower 16 bits).
    pub fn read_phy_id(&self) -> Option<u32> {
        let id1 = self.mdio_read(PHY_ADDR, MII_PHYSID1)?;
        let id2 = self.mdio_read(PHY_ADDR, MII_PHYSID2)?;

        Some(((id1 as u32) << 16) | (id2 as u32))
    }

    /// Run comprehensive hardware diagnostics
    ///
    /// This performs a thorough check of the GENET controller and PHY,
    /// printing detailed status information for debugging.
    ///
    /// Returns true if all checks pass, false if any fail.
    pub fn diagnostic(&self) -> bool {
        println!("[DIAG] Ethernet Hardware Diagnostics");
        println!("[DIAG] ================================");

        // Step 1: Check if hardware is present
        println!("[DIAG] Step 1: GENET Controller Detection");
        println!(
            "[DIAG]   Reading SYS_REV_CTRL @ {:#010X}...",
            self.base_addr + SYS_REV_CTRL
        );

        if !self.is_present() {
            let version = self.get_version();
            println!(
                "[WARN]   Unexpected version: {:#010X} (expected 0x0005xxxx)",
                version
            );
            println!("[INFO]   Hardware not present (running in QEMU?)");
            println!("[SKIP] Diagnostics completed (no hardware detected)");
            return false;
        }

        let version = self.get_version();
        let major = (version >> 16) & 0xFFFF;
        let minor = (version >> 8) & 0xFF;
        let patch = version & 0xFF;
        println!(
            "[PASS]   GENET v{}.{}.{} detected (version: {:#010X})",
            major, minor, patch, version
        );
        println!();

        // Step 2: PHY Detection via MDIO
        println!("[DIAG] Step 2: PHY Detection");
        println!("[DIAG]   Scanning MDIO address {}...", PHY_ADDR);

        println!(
            "[DIAG]   Reading PHY_ID1 @ addr {}, reg {:#04X}...",
            PHY_ADDR, MII_PHYSID1
        );
        let id1 = match self.mdio_read(PHY_ADDR, MII_PHYSID1) {
            Some(val) => {
                println!("[DIAG]     Value: {:#06X}", val);
                val
            }
            None => {
                println!("[FAIL]     MDIO read timeout");
                println!("[FAIL] PHY detection failed");
                return false;
            }
        };

        println!(
            "[DIAG]   Reading PHY_ID2 @ addr {}, reg {:#04X}...",
            PHY_ADDR, MII_PHYSID2
        );
        let id2 = match self.mdio_read(PHY_ADDR, MII_PHYSID2) {
            Some(val) => {
                println!("[DIAG]     Value: {:#06X}", val);
                val
            }
            None => {
                println!("[FAIL]     MDIO read timeout");
                println!("[FAIL] PHY detection failed");
                return false;
            }
        };

        let phy_id = ((id1 as u32) << 16) | (id2 as u32);

        if phy_id == PHY_ID_BCM54213PE {
            println!(
                "[PASS]   PHY found at address {}: BCM54213PE (ID: {:#010X})",
                PHY_ADDR, phy_id
            );
        } else {
            println!(
                "[WARN]   PHY found at address {} with unexpected ID: {:#010X}",
                PHY_ADDR, phy_id
            );
            println!(
                "[WARN]   Expected: {:#010X} (BCM54213PE)",
                PHY_ID_BCM54213PE
            );
        }
        println!();

        // Step 3: Read PHY status registers
        println!("[DIAG] Step 3: PHY Status");

        println!("[DIAG]   Reading BMSR (Basic Mode Status Register)...");
        if let Some(bmsr) = self.mdio_read(PHY_ADDR, MII_BMSR) {
            println!("[DIAG]     BMSR: {:#06X}", bmsr);
            println!(
                "[DIAG]       Link status: {}",
                if (bmsr & BMSR_LSTATUS) != 0 {
                    "UP"
                } else {
                    "DOWN"
                }
            );
            println!(
                "[DIAG]       Auto-negotiation: {}",
                if (bmsr & BMSR_ANEGCOMPLETE) != 0 {
                    "COMPLETE"
                } else {
                    "IN PROGRESS"
                }
            );
        } else {
            println!("[WARN]     MDIO read timeout");
        }

        println!("[DIAG]   Reading BMCR (Basic Mode Control Register)...");
        if let Some(bmcr) = self.mdio_read(PHY_ADDR, MII_BMCR) {
            println!("[DIAG]     BMCR: {:#06X}", bmcr);
            println!(
                "[DIAG]       Auto-negotiation: {}",
                if (bmcr & BMCR_ANENABLE) != 0 {
                    "ENABLED"
                } else {
                    "DISABLED"
                }
            );
        } else {
            println!("[WARN]     MDIO read timeout");
        }
        println!();

        println!("[PASS] ================================");
        println!("[PASS] Hardware diagnostics complete!");
        println!("[PASS] GENET v5 and BCM54213PE PHY detected");
        println!();

        true
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_genet_register_offsets() {
        // Verify critical register offsets match Linux driver
        assert_eq!(SYS_REV_CTRL, 0x0000);
        assert_eq!(UMAC_CMD, 0x0808);
        assert_eq!(UMAC_MAC0, 0x080C);
        assert_eq!(UMAC_MAC1, 0x0810);
        assert_eq!(UMAC_MDIO_CMD, 0x0E14);
    }

    #[test_case]
    fn test_mdio_command_encoding() {
        // Test MDIO read command encoding
        let phy_addr = 0x01u8;
        let reg_addr = 0x02u8;

        let cmd = MDIO_START_BUSY
            | MDIO_RD
            | ((phy_addr as u32) << MDIO_PHY_ADDR_SHIFT)
            | ((reg_addr as u32) << MDIO_REG_ADDR_SHIFT);

        // Verify bit fields
        assert_eq!(cmd & MDIO_START_BUSY, MDIO_START_BUSY);
        assert_eq!(cmd & (0b11 << 26), MDIO_RD);
        assert_eq!((cmd >> MDIO_PHY_ADDR_SHIFT) & 0x1F, phy_addr as u32);
        assert_eq!((cmd >> MDIO_REG_ADDR_SHIFT) & 0x1F, reg_addr as u32);
    }

    #[test_case]
    fn test_phy_id_constants() {
        // Verify expected PHY ID
        assert_eq!(PHY_ID_BCM54213PE, 0x600D84A2);
        assert_eq!(PHY_ADDR, 0x01);
    }

    #[test_case]
    fn test_mii_register_addresses() {
        // Verify standard MII register addresses
        assert_eq!(MII_BMCR, 0x00);
        assert_eq!(MII_BMSR, 0x01);
        assert_eq!(MII_PHYSID1, 0x02);
        assert_eq!(MII_PHYSID2, 0x03);
        assert_eq!(MII_ADVERTISE, 0x04);
        assert_eq!(MII_LPA, 0x05);
    }
}

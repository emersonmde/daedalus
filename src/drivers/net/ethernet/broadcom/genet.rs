//! GENET v5 Ethernet Controller Driver (Raspberry Pi 4)
//!
//! This driver implements support for the Broadcom GENET v5 MAC controller
//! found on the BCM2711 SoC (Raspberry Pi 4). It provides basic TX/RX
//! functionality using DMA ring buffers.
//!
//! # Implementation Notes
//!
//! This driver closely follows U-Boot's bcmgenet.c implementation to ensure
//! hardware compatibility. Key design decisions:
//!
//! - Ring 16 (default queue) for both TX and RX
//! - Polling mode (no interrupts)
//! - Single TX/RX buffer per direction
//! - External PHY (BCM54213PE) via RGMII
//!
//! # References
//!
//! - U-Boot: drivers/net/bcmgenet.c
//! - Linux: drivers/net/ethernet/broadcom/genet/bcmgenet.c
//! - Datasheet: BCM2711 ARM Peripherals (no public GENET docs)

use crate::drivers::clocksource::SystemTimer;
use crate::drivers::netdev::{NetworkDevice, NetworkError};
use crate::net::ethernet::MacAddress;
use crate::println;
use alloc::boxed::Box;

// ============================================================================
// Hardware Constants
// ============================================================================

/// GENET controller base address (BCM2711)
///
/// ARM physical address: 0xFD580000 (bus address 0x7D580000 + 0x80000000)
/// Source: BCM2711 ARM Peripherals, Section 2.1
///
/// Public for device tree verification (Phase 3 - comparing DT vs hardcoded)
/// Will be replaced by DTB value in Phase 4
pub const GENET_BASE: usize = 0xFD58_0000;

// NOTE: DMA_BUS_OFFSET (0xC0000000) is NOT used by GENET
// U-Boot writes direct physical addresses to descriptors
// This offset is used by other BCM2711 DMA controllers, but not GENET

/// Ring size (number of descriptors)
/// Source: U-Boot uses 256 descriptors for TX and RX
const RING_SIZE: usize = 256;

/// DMA descriptor size in bytes
const DMA_DESC_SIZE: usize = 12;

/// Maximum Ethernet frame size (standard MTU)
const MAX_FRAME_SIZE: usize = 1536; // U-Boot uses ENET_MAX_MTU_SIZE

/// Minimum Ethernet frame size
const MIN_FRAME_SIZE: usize = 60;

/// RX buffer size per descriptor
const RX_BUF_LENGTH: usize = 2048;

/// Total RX buffer size (256 descriptors * 2048 bytes each)
const RX_TOTAL_BUFSIZE: usize = RING_SIZE * RX_BUF_LENGTH;

/// RX buffer offset (2-byte padding when RBUF_ALIGN_2B is set)
const RX_BUF_OFFSET: usize = 2;

// ============================================================================
// Register Block Offsets
// ============================================================================

const SYS_OFF: usize = 0x0000;
const EXT_OFF: usize = 0x0080;
const INTRL2_0_OFF: usize = 0x0200;
const INTRL2_1_OFF: usize = 0x0240;
const RBUF_OFF: usize = 0x0300;
const UMAC_OFF: usize = 0x0800;

/// TX DMA registers base
const TDMA_OFF: usize = 0x4000;
/// RX DMA registers base
const RDMA_OFF: usize = 0x2000;

/// TX descriptor base offset
/// Source: U-Boot GENET_TX_OFF (descriptors start at DMA register base)
const TDMA_DESC_OFF: usize = 0x4000;
/// RX descriptor base offset
/// Source: U-Boot GENET_RX_OFF (descriptors start at DMA register base)
const RDMA_DESC_OFF: usize = 0x2000;

// ============================================================================
// System Registers (SYS_OFF)
// ============================================================================

const SYS_REV_CTRL: usize = SYS_OFF;
const SYS_PORT_CTRL: usize = SYS_OFF + 0x04;
const SYS_RBUF_FLUSH_CTRL: usize = SYS_OFF + 0x08;

// SYS_PORT_CTRL bits
const PORT_MODE_EXT_GPHY: u32 = 3; // External GPHY mode

// SYS_RBUF_FLUSH_CTRL bits
const SYS_RBUF_FLUSH_RESET: u32 = 1 << 1;

// ============================================================================
// EXT Block Registers
// ============================================================================

const EXT_PWR_MGMT: usize = EXT_OFF;
const EXT_RGMII_OOB_CTRL: usize = EXT_OFF + 0x0C;

// EXT_PWR_MGMT bits
const EXT_PWR_DOWN_PHY: u32 = 1 << 0;
const EXT_PWR_DOWN_DLL: u32 = 1 << 1;
const EXT_PWR_DOWN_BIAS: u32 = 1 << 2;
const EXT_ENERGY_DET_MASK: u32 = 0x1F << 4;

// EXT_RGMII_OOB_CTRL bits
const EXT_RGMII_OOB_RGMII_MODE_EN: u32 = 1 << 6;
const EXT_RGMII_OOB_ID_MODE_DISABLE: u32 = 1 << 16;
const EXT_RGMII_OOB_OOB_DISABLE: u32 = 1 << 5;

// ============================================================================
// RBUF Registers
// ============================================================================

const RBUF_CTRL: usize = RBUF_OFF;
const RBUF_TBUF_SIZE_CTRL: usize = RBUF_OFF + 0xB4;

// RBUF_CTRL bits
#[allow(dead_code)] // Future use: Enable 64-byte buffer alignment (not needed - using 2-byte alignment)
const RBUF_64B_EN: u32 = 1 << 0;
const RBUF_ALIGN_2B: u32 = 1 << 1;

// TBUF_CTRL register (future use for TX buffer configuration)
#[allow(dead_code)] // Future use: TX buffer control register
const TBUF_CTRL: usize = 0x0B00;
#[allow(dead_code)] // Future use: Enable 64-byte TX buffer mode
const TBUF_64B_EN: u32 = 1 << 0;

// ============================================================================
// INTRL2 Registers
// ============================================================================

// INTRL2 register offsets (source: Linux kernel drivers/net/ethernet/broadcom/genet/bcmgenet.h)
const INTRL2_CPU_STAT: usize = 0x00; // Interrupt status (read-only)
#[allow(dead_code)] // Software interrupt trigger (not currently used)
const INTRL2_CPU_SET: usize = 0x04; // Set interrupt bits (write-only, triggers interrupt)
const INTRL2_CPU_CLEAR: usize = 0x08; // Clear interrupt bits (write-only, ack interrupt)
const INTRL2_CPU_MASK_STATUS: usize = 0x0C; // Current mask state (read-only, 1=masked)
const INTRL2_CPU_MASK_SET: usize = 0x10; // Set mask bits (write-only, mask=disable interrupt)
const INTRL2_CPU_MASK_CLEAR: usize = 0x14; // Clear mask bits (write-only, unmask=enable interrupt)

// INTRL2_0 interrupt bits (source: Linux kernel drivers/net/ethernet/broadcom/genet/bcmgenet.h)
const UMAC_IRQ_RXDMA_MBDONE: u32 = 1 << 13; // RX DMA descriptor done
#[allow(dead_code)] // Future use for TX interrupt handling
const UMAC_IRQ_TXDMA_MBDONE: u32 = 1 << 16; // TX DMA descriptor done

// ============================================================================
// HFB (Hardware Filter Block) Registers - GENETv5 only
// ============================================================================
// Source: Linux kernel drivers/net/ethernet/broadcom/genet/bcmgenet.h
// The HFB filters packets in hardware before they reach RX descriptors,
// preventing interrupt storms from unwanted traffic on busy networks.

const GENET_HFB_OFF: usize = 0x8000;

// HFB control registers (relative to GENET_HFB_OFF)
const HFB_CTRL: usize = 0x00; // Enable/disable HFB
const HFB_FLT_ENABLE_V3PLUS: usize = 0x04; // 32-bit filter enable mask
const HFB_FLT_LEN_V3PLUS: [usize; 8] = [
    // Filter length registers (4 filters each)
    0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x28,
];
const HFB_RXNFC_LKUP_CTRL: usize = 0x88; // Action on match (base, +4 per filter)

// Filter memory starts at offset 0x200 from HFB base (32 bytes per filter)
const HFB_FLT_BASE: usize = 0x200;

// ============================================================================
// UMAC Registers
// ============================================================================

const UMAC_CMD: usize = UMAC_OFF + 0x008;
const UMAC_MAC0: usize = UMAC_OFF + 0x00C;
const UMAC_MAC1: usize = UMAC_OFF + 0x010;
const UMAC_MAX_FRAME_LEN: usize = UMAC_OFF + 0x014;
const UMAC_MODE: usize = UMAC_OFF + 0x044;
const UMAC_MIB_CTRL: usize = UMAC_OFF + 0x580;
const UMAC_MDIO_CMD: usize = UMAC_OFF + 0x614;

// UMAC_CMD bits
const CMD_TX_EN: u32 = 1 << 0;
const CMD_RX_EN: u32 = 1 << 1;
const CMD_SPEED_SHIFT: u32 = 2;
const CMD_SPEED_10: u32 = 0 << CMD_SPEED_SHIFT;
const CMD_SPEED_100: u32 = 1 << CMD_SPEED_SHIFT;
const CMD_SPEED_1000: u32 = 2 << CMD_SPEED_SHIFT;
#[allow(dead_code)] // Future use: Enable promiscuous mode (accept all packets, useful for packet sniffing)
const CMD_PROMISC: u32 = 1 << 4;
const CMD_SW_RESET: u32 = 1 << 13;
const CMD_LCL_LOOP_EN: u32 = 1 << 15;

// UMAC_MODE bits
const MODE_FULL_DUPLEX: u32 = 1 << 0;

// UMAC_MIB_CTRL bits
const MIB_RESET_RX: u32 = 1 << 0;
const MIB_RESET_TX: u32 = 1 << 2;
const MIB_RESET_RUNT: u32 = 1 << 1;

// MIB Counter Registers
const MIB_BASE: usize = UMAC_OFF + 0x400;
const MIB_TX_GOOD_PKTS: usize = UMAC_OFF + 0x4A8;
const MIB_TX_GOOD_OCTETS: usize = UMAC_OFF + 0x4C0;
const MIB_TX_MCAST_PKTS: usize = UMAC_OFF + 0x4AC;
const MIB_TX_BCAST_PKTS: usize = UMAC_OFF + 0x4B0;
const MIB_RX_GOOD_PKTS: usize = MIB_BASE + 0x08;
const MIB_RX_GOOD_OCTETS: usize = MIB_BASE + 0xC0;
const MIB_RX_UCAST_PKTS: usize = MIB_BASE + 0xD0;
const MIB_RX_MCAST_PKTS: usize = MIB_BASE + 0x0C;
const MIB_RX_BCAST_PKTS: usize = MIB_BASE + 0x10;
const MIB_RX_FCS_ERR: usize = MIB_BASE + 0x28;
const MIB_RX_ALIGN_ERR: usize = MIB_BASE + 0x30;

// ============================================================================
// MDIO Registers (for PHY access)
// ============================================================================

// UMAC_MDIO_CMD bits
const MDIO_START_BUSY: u32 = 1 << 29;
const MDIO_READ_FAIL: u32 = 1 << 28;
const MDIO_RD: u32 = 2 << 26;
#[allow(dead_code)] // Future use: PHY register writes (mdio_write function not yet implemented)
const MDIO_WR: u32 = 1 << 26;
const MDIO_PMD_SHIFT: u32 = 21;
const MDIO_REG_SHIFT: u32 = 16;

// PHY address (external PHY on Pi 4)
const PHY_ADDR: u32 = 1;

// Standard MII registers
const MII_BMSR: u32 = 0x01;
const MII_PHYSID1: u32 = 0x02;
const MII_PHYSID2: u32 = 0x03;
const MII_LPA: u32 = 0x05;

// BMSR bits
const BMSR_LSTATUS: u16 = 1 << 2;
const BMSR_ANEGCOMPLETE: u16 = 1 << 5;

// LPA bits (Link Partner Ability)
const LPA_100FULL: u16 = 1 << 8;
const LPA_100HALF: u16 = 1 << 7;
const LPA_10FULL: u16 = 1 << 6;
const LPA_10HALF: u16 = 1 << 5;

// 1000BASE-T registers
const MII_STAT1000: u32 = 0x0A;
const LPA_1000FULL: u16 = 1 << 11;
#[allow(dead_code)] // Hardware spec: 1000 Mbps half-duplex (not used - Pi 4 uses full-duplex)
const LPA_1000HALF: u16 = 1 << 10;

// ============================================================================
// DMA Registers
// ============================================================================

// Ring 16 (default queue) offsets
const DESC_INDEX: u32 = 16;

// TX DMA Ring 16 (source: U-Boot TDMA_RING_REG_BASE + offsets)
// Ring base = TDMA_OFF + descriptors (0xC00) + ring_offset (16 * 0x40 = 0x400) = 0x5000
const TDMA_RING16_READ_PTR: usize = TDMA_OFF + 0x1000;
const TDMA_RING16_CONS_INDEX: usize = TDMA_OFF + 0x1000 + 0x08;
const TDMA_RING16_PROD_INDEX: usize = TDMA_OFF + 0x1000 + 0x0C;
const TDMA_RING16_SIZE: usize = TDMA_OFF + 0x1000 + 0x10; // RING_BUF_SIZE
const TDMA_RING16_START_ADDR: usize = TDMA_OFF + 0x1000 + 0x14;
const TDMA_RING16_END_ADDR: usize = TDMA_OFF + 0x1000 + 0x1C;
const TDMA_RING16_MBUF_DONE_THRESH: usize = TDMA_OFF + 0x1000 + 0x24;
const TDMA_RING16_FLOW_PERIOD: usize = TDMA_OFF + 0x1000 + 0x28;
const TDMA_RING16_WRITE_PTR: usize = TDMA_OFF + 0x1000 + 0x2C;

// RX DMA Ring 16 (source: U-Boot RDMA_RING_REG_BASE + offsets)
// Ring base = RDMA_OFF + descriptors (0xC00) + ring_offset (16 * 0x40 = 0x400) = 0x3000
const RDMA_RING16_WRITE_PTR: usize = RDMA_OFF + 0x1000;
const RDMA_RING16_PROD_INDEX: usize = RDMA_OFF + 0x1000 + 0x08;
const RDMA_RING16_CONS_INDEX: usize = RDMA_OFF + 0x1000 + 0x0C;
const RDMA_RING16_SIZE: usize = RDMA_OFF + 0x1000 + 0x10; // RING_BUF_SIZE
const RDMA_RING16_START_ADDR: usize = RDMA_OFF + 0x1000 + 0x14;
const RDMA_RING16_END_ADDR: usize = RDMA_OFF + 0x1000 + 0x1C;
#[allow(dead_code)] // Hardware spec: RX MBUF threshold (not used - U-Boot doesn't set for RX)
const RDMA_RING16_MBUF_DONE_THRESH: usize = RDMA_OFF + 0x1000 + 0x24;
const RDMA_RING16_XON_XOFF_THRESH: usize = RDMA_OFF + 0x1000 + 0x28;
const RDMA_RING16_READ_PTR: usize = RDMA_OFF + 0x1000 + 0x2C;

// Global DMA control registers
// Located after descriptor area (256*12=0xC00) and ring configs (17*0x40=0x440)
const TDMA_REG_BASE: usize = TDMA_OFF + 0x1040;
const RDMA_REG_BASE: usize = RDMA_OFF + 0x1040;

const TDMA_RING_CFG: usize = TDMA_REG_BASE; // Global ring enable register
const TDMA_CTRL: usize = TDMA_REG_BASE + 0x04;
const TDMA_SCB_BURST_SIZE: usize = TDMA_REG_BASE + 0x0C;

const RDMA_RING_CFG: usize = RDMA_REG_BASE; // Global ring enable register
const RDMA_CTRL: usize = RDMA_REG_BASE + 0x04;
const RDMA_SCB_BURST_SIZE: usize = RDMA_REG_BASE + 0x0C;

// DMA control bits
const DMA_CTRL_EN: u32 = 1 << 0;
const DMA_RING_BUF_EN_SHIFT: u32 = 1;

// DMA burst size
const DMA_MAX_BURST_LENGTH: u32 = 0x08;

// ============================================================================
// DMA Descriptor Fields
// ============================================================================
// Source: U-Boot bcmgenet.c - descriptor field order

const DMA_DESC_LENGTH_STATUS: usize = 0x00; // Length/status at offset 0
const DMA_DESC_ADDRESS_LO: usize = 0x04; // Low 32 bits of address at offset 4
const DMA_DESC_ADDRESS_HI: usize = 0x08; // High 32 bits of address at offset 8

// Length/Status field bits
// Source: U-Boot bcmgenet.c DMA descriptor flags
const DMA_BUFLENGTH_SHIFT: u32 = 16;
const DMA_BUFLENGTH_MASK: u32 = 0x0FFF;
const DMA_OWN: u32 = 0x8000; // bit 15: Hardware owns descriptor
const DMA_EOP: u32 = 0x4000; // bit 14: End of packet
const DMA_SOP: u32 = 0x2000; // bit 13: Start of packet
#[allow(dead_code)] // Hardware spec: Wrap flag (not used - implicit via modulo arithmetic)
const DMA_WRAP: u32 = 0x1000; // bit 12: Wrap to start of ring
const DMA_TX_APPEND_CRC: u32 = 0x0040; // bit 6: Append CRC
const DMA_TX_QTAG_SHIFT: u32 = 7;

// ============================================================================
// Cache Management Constants
// ============================================================================

/// ARM Cortex-A72 cache line size (BCM2711)
const CACHE_LINE_SIZE: usize = 64;

// ============================================================================
// DMA Descriptor Constants
// ============================================================================

/// DMA descriptor word count (12 bytes = 3 words)
/// Source: U-Boot descriptor size / sizeof(u32)
const DMA_DESC_WORDS: u32 = (DMA_DESC_SIZE / 4) as u32;

// ============================================================================
// Driver State
// ============================================================================

pub struct GenetController {
    base_addr: usize,
    tx_index: usize,
    tx_prod_index: u32, // TX producer index (tracks what we've queued)
    rx_index: usize,
    rx_c_index: usize, // RX consumer index (for tracking processed packets)

    // Single TX buffer (U-Boot uses caller's buffer, we copy for simplicity)
    tx_buffer: [u8; MAX_FRAME_SIZE],

    // RX buffer array (256 * 2KB = 512KB total)
    rxbuffer: Box<[u8; RX_TOTAL_BUFSIZE]>,

    mac_address: MacAddress,
    initialized: bool,
}

#[allow(clippy::new_without_default)] // Hardware controllers shouldn't have Default - explicit new() is clearer
impl GenetController {
    /// Create new GENET controller instance
    ///
    /// Note: MAC address is initialized to zero and will be set during init()
    /// by querying the VideoCore firmware via mailbox.
    pub fn new() -> Self {
        Self::with_base_addr(GENET_BASE)
    }

    /// Create GENET controller with custom base address (from device tree)
    ///
    /// # Arguments
    /// * `base_addr` - ARM physical address for GENET controller (e.g., 0xFD580000)
    ///
    /// This constructor allows runtime configuration from device tree.
    /// The default `new()` uses the hardcoded GENET_BASE constant.
    pub fn with_base_addr(base_addr: usize) -> Self {
        Self {
            base_addr,
            tx_index: 0,
            tx_prod_index: 0,
            rx_index: 0,
            rx_c_index: 0,
            tx_buffer: [0u8; MAX_FRAME_SIZE],
            rxbuffer: Box::new([0u8; RX_TOTAL_BUFSIZE]),
            mac_address: MacAddress::new([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            initialized: false,
        }
    }

    // ========================================================================
    // Register Access
    // ========================================================================

    #[inline]
    fn read_reg(&self, offset: usize) -> u32 {
        let addr = (self.base_addr + offset) as *const u32;
        // SAFETY: GENET registers are memory-mapped at valid addresses
        unsafe { core::ptr::read_volatile(addr) }
    }

    #[inline]
    fn write_reg(&self, offset: usize, value: u32) {
        // SAFETY: Data Memory Barrier ensures all memory accesses before this write
        // are observed before MMIO register write. This prevents CPU reordering that
        // could cause DMA descriptors to be read before their buffers are written.
        // Matches U-Boot's __iowmb() pattern.
        unsafe {
            core::arch::asm!("dmb sy", options(nostack));
        }
        let addr = (self.base_addr + offset) as *mut u32;
        // SAFETY: GENET registers are memory-mapped at valid addresses
        unsafe { core::ptr::write_volatile(addr, value) }
    }

    // ========================================================================
    // Cache Management Helpers
    // ========================================================================

    /// Flush cache for a memory region (DC CVAC - Clean by VA to PoC)
    ///
    /// Ensures CPU writes are flushed to DRAM so DMA can see them.
    /// Used before TX to flush packet buffers.
    ///
    /// # Safety
    /// Caller must ensure the memory region [start_addr, end_addr) is valid.
    #[inline]
    fn cache_flush(&self, start_addr: usize, length: usize) {
        let end_addr = start_addr + length;
        let start_aligned = start_addr & !(CACHE_LINE_SIZE - 1);
        let end_aligned = (end_addr + CACHE_LINE_SIZE - 1) & !(CACHE_LINE_SIZE - 1);

        // SAFETY: Flushing cache for valid memory region.
        // DC CVAC cleans (writes back) dirty cache lines without invalidating.
        // DSB ensures cache operations complete before proceeding.
        unsafe {
            let mut addr = start_aligned;
            while addr < end_aligned {
                core::arch::asm!(
                    "dc cvac, {addr}",
                    addr = in(reg) addr,
                    options(nostack)
                );
                addr += CACHE_LINE_SIZE;
            }
            core::arch::asm!("dsb sy", options(nostack));
        }
    }

    /// Invalidate cache for a memory region (DC IVAC - Invalidate by VA to PoC)
    ///
    /// Discards cached data so CPU will read from DRAM where DMA wrote.
    /// Used after RX to invalidate stale cached data and before RX init to
    /// ensure DMA writes go directly to DRAM.
    ///
    /// # Safety
    /// Caller must ensure the memory region [start_addr, end_addr) is valid.
    #[inline]
    fn cache_invalidate(&self, start_addr: usize, length: usize) {
        let end_addr = start_addr + length;
        let start_aligned = start_addr & !(CACHE_LINE_SIZE - 1);
        let end_aligned = (end_addr + CACHE_LINE_SIZE - 1) & !(CACHE_LINE_SIZE - 1);

        // SAFETY: Invalidating cache for valid memory region.
        // DC IVAC invalidates cache lines without writing back (data loss if dirty!).
        // DSB ensures cache operations complete and memory ordering is preserved.
        unsafe {
            core::arch::asm!("dsb sy", options(nostack));
            let mut addr = start_aligned;
            while addr < end_aligned {
                core::arch::asm!(
                    "dc ivac, {addr}",
                    addr = in(reg) addr,
                    options(nostack)
                );
                addr += CACHE_LINE_SIZE;
            }
            core::arch::asm!("dsb sy", options(nostack));
        }
    }

    // ========================================================================
    // MDIO/PHY Operations
    // ========================================================================

    fn mdio_wait(&self) {
        for _ in 0..1000 {
            if (self.read_reg(UMAC_MDIO_CMD) & MDIO_START_BUSY) == 0 {
                return;
            }
            SystemTimer::delay_us(10);
        }
    }

    fn mdio_read(&self, phy: u32, reg: u32) -> Option<u16> {
        self.mdio_wait();

        let cmd = MDIO_START_BUSY | MDIO_RD | (phy << MDIO_PMD_SHIFT) | (reg << MDIO_REG_SHIFT);

        self.write_reg(UMAC_MDIO_CMD, cmd);
        self.mdio_wait();

        let val = self.read_reg(UMAC_MDIO_CMD);
        if (val & MDIO_READ_FAIL) != 0 {
            None
        } else {
            Some((val & 0xFFFF) as u16)
        }
    }

    /// Read PHY identifier (for diagnostics)
    ///
    /// Returns the full 32-bit PHY ID by reading PHYSID1 (upper 16 bits)
    /// and PHYSID2 (lower 16 bits).
    ///
    /// For BCM54213PE: 0x600D84A2
    #[allow(dead_code)]
    fn read_phy_id(&self) -> Option<u32> {
        let id1 = self.mdio_read(PHY_ADDR, MII_PHYSID1)? as u32;
        let id2 = self.mdio_read(PHY_ADDR, MII_PHYSID2)? as u32;
        Some((id1 << 16) | id2)
    }

    fn read_link_params(&self) -> Option<LinkParams> {
        // Read BMSR to check link status
        let bmsr = self.mdio_read(PHY_ADDR, MII_BMSR)?;
        if (bmsr & BMSR_LSTATUS) == 0 {
            return None; // No link
        }

        // Check 1000BASE-T status first
        if let Some(stat1000) = self.mdio_read(PHY_ADDR, MII_STAT1000)
            && (stat1000 & LPA_1000FULL) != 0
        {
            return Some(LinkParams {
                speed: LinkSpeed::Speed1000,
                duplex: DuplexMode::Full,
            });
        }

        // Check 10/100 capabilities
        if let Some(lpa) = self.mdio_read(PHY_ADDR, MII_LPA) {
            if (lpa & LPA_100FULL) != 0 {
                return Some(LinkParams {
                    speed: LinkSpeed::Speed100,
                    duplex: DuplexMode::Full,
                });
            } else if (lpa & LPA_100HALF) != 0 {
                return Some(LinkParams {
                    speed: LinkSpeed::Speed100,
                    duplex: DuplexMode::Half,
                });
            } else if (lpa & LPA_10FULL) != 0 {
                return Some(LinkParams {
                    speed: LinkSpeed::Speed10,
                    duplex: DuplexMode::Full,
                });
            } else if (lpa & LPA_10HALF) != 0 {
                return Some(LinkParams {
                    speed: LinkSpeed::Speed10,
                    duplex: DuplexMode::Half,
                });
            }
        }

        None
    }

    // ========================================================================
    // Hardware Detection
    // ========================================================================

    pub fn is_present(&self) -> bool {
        let rev = self.read_reg(SYS_REV_CTRL);
        let major = (rev >> 24) & 0x0F;

        // BCM2711 (Pi 4) reports major version 6 in hardware register
        // but is documented as GENET v5. Linux normalizes 6→5 and 7→5.
        // Source: Linux kernel drivers/net/ethernet/broadcom/genet/bcmgenet.c::bcmgenet_set_hw_params()
        major == 5 || major == 6 || major == 7
    }

    // ========================================================================
    // Initialization Functions (following U-Boot's exact sequence)
    // ========================================================================

    /// UMAC reset sequence
    /// Source: U-Boot bcmgenet.c::bcmgenet_umac_reset()
    fn umac_reset(&mut self) {
        // Step 1: Flush RX buffer
        let mut reg = self.read_reg(SYS_RBUF_FLUSH_CTRL);
        reg |= SYS_RBUF_FLUSH_RESET;
        self.write_reg(SYS_RBUF_FLUSH_CTRL, reg);
        SystemTimer::delay_us(10);

        reg &= !SYS_RBUF_FLUSH_RESET;
        self.write_reg(SYS_RBUF_FLUSH_CTRL, reg);
        SystemTimer::delay_us(10);

        self.write_reg(SYS_RBUF_FLUSH_CTRL, 0);
        SystemTimer::delay_us(10);

        // Step 2: Clear UMAC_CMD
        self.write_reg(UMAC_CMD, 0);

        // Step 3: Software reset with local loopback
        self.write_reg(UMAC_CMD, CMD_SW_RESET | CMD_LCL_LOOP_EN);
        SystemTimer::delay_us(2);

        // Step 4: Clear reset
        self.write_reg(UMAC_CMD, 0);

        // Step 5: Reset MIB counters
        self.write_reg(UMAC_MIB_CTRL, MIB_RESET_RX | MIB_RESET_TX | MIB_RESET_RUNT);
        self.write_reg(UMAC_MIB_CTRL, 0);

        // Step 6: Set max frame length
        self.write_reg(UMAC_MAX_FRAME_LEN, MAX_FRAME_SIZE as u32);

        // Step 7: Configure RX buffer alignment
        let mut rbuf_ctrl = self.read_reg(RBUF_CTRL);
        rbuf_ctrl |= RBUF_ALIGN_2B;
        self.write_reg(RBUF_CTRL, rbuf_ctrl);

        // Step 8: Set TBUF size (U-Boot sets this to 1)
        self.write_reg(RBUF_TBUF_SIZE_CTRL, 1);
    }

    /// Initialize RX descriptors
    /// Source: U-Boot bcmgenet.c::rx_descs_init()
    fn rx_descs_init(&mut self) {
        let rxbuffers_base = self.rxbuffer.as_ptr() as usize;

        // Invalidate cache for all RX buffers BEFORE setting up descriptors
        // This ensures any stale cached data (like zeros from allocation) is discarded
        // so DMA writes go directly to DRAM
        let total_rx_size = RING_SIZE * RX_BUF_LENGTH;
        self.cache_invalidate(rxbuffers_base, total_rx_size);

        for i in 0..RING_SIZE {
            let buffer_offset = i * RX_BUF_LENGTH;
            let buffer_addr = (rxbuffers_base + buffer_offset) as u32;

            let desc_offset = RDMA_DESC_OFF + (i * DMA_DESC_SIZE);

            // Write descriptor fields
            // Source: U-Boot writes direct pointer, NO DMA_BUS_OFFSET
            self.write_reg(desc_offset + DMA_DESC_ADDRESS_LO, buffer_addr);
            self.write_reg(desc_offset + DMA_DESC_ADDRESS_HI, 0);

            let len_status = ((RX_BUF_LENGTH as u32) << DMA_BUFLENGTH_SHIFT) | DMA_OWN;
            self.write_reg(desc_offset + DMA_DESC_LENGTH_STATUS, len_status);
        }
    }

    /// Initialize TX ring
    /// Source: U-Boot bcmgenet.c::tx_ring_init()
    fn tx_ring_init(&mut self) {
        self.write_reg(TDMA_RING16_START_ADDR, 0);
        self.write_reg(TDMA_RING16_READ_PTR, 0);
        self.write_reg(TDMA_RING16_WRITE_PTR, 0);

        // End address is in words (4-byte units), not bytes
        let end_addr = ((RING_SIZE as u32) * DMA_DESC_WORDS) - 1;
        self.write_reg(TDMA_RING16_END_ADDR, end_addr);

        let buf_size = ((RING_SIZE as u32) << 16) | (RX_BUF_LENGTH as u32);
        self.write_reg(TDMA_RING16_SIZE, buf_size);

        // Align PROD_INDEX to CONS_INDEX (can't reset CONS to 0)
        let cons_index = self.read_reg(TDMA_RING16_CONS_INDEX);
        self.write_reg(TDMA_RING16_PROD_INDEX, cons_index);
        self.tx_index = (cons_index & 0xFFFF) as usize;
        self.tx_prod_index = cons_index;

        self.write_reg(TDMA_RING16_MBUF_DONE_THRESH, 1);
        self.write_reg(TDMA_RING16_FLOW_PERIOD, 0);

        // Enable ring 16 in global DMA ring configuration
        // Source: U-Boot writes (1 << DEFAULT_Q) to TDMA_REG_BASE + DMA_RING_CFG
        self.write_reg(TDMA_RING_CFG, 1 << DESC_INDEX);
    }

    /// Initialize RX ring
    /// Source: U-Boot bcmgenet.c::rx_ring_init()
    fn rx_ring_init(&mut self) {
        self.write_reg(RDMA_RING16_START_ADDR, 0);
        self.write_reg(RDMA_RING16_READ_PTR, 0);
        self.write_reg(RDMA_RING16_WRITE_PTR, 0);

        // End address is in words (4-byte units), not bytes
        let end_addr = ((RING_SIZE as u32) * DMA_DESC_WORDS) - 1;
        self.write_reg(RDMA_RING16_END_ADDR, end_addr);

        let buf_size = ((RING_SIZE as u32) << 16) | (RX_BUF_LENGTH as u32);
        self.write_reg(RDMA_RING16_SIZE, buf_size);

        // Align CONS_INDEX to PROD_INDEX (can't reset PROD to 0)
        let prod_index = self.read_reg(RDMA_RING16_PROD_INDEX);
        self.write_reg(RDMA_RING16_CONS_INDEX, prod_index);
        self.rx_index = (prod_index & 0xFFFF) as usize;
        self.rx_c_index = self.rx_index;

        // Note: RX ring doesn't have MBUF_DONE_THRESH or FLOW_PERIOD writes in U-Boot

        // Set RX flow control thresholds (critical for RX DMA)
        // Source: U-Boot bcmgenet.c::rx_ring_init() - RDMA_XON_XOFF_THRESH
        let xon_xoff_thresh = (5 << 16) | ((RING_SIZE >> 4) as u32);
        self.write_reg(RDMA_RING16_XON_XOFF_THRESH, xon_xoff_thresh);

        // Enable ring 16 in global DMA ring configuration
        // Source: U-Boot writes (1 << DEFAULT_Q) to RDMA_REG_BASE + DMA_RING_CFG
        self.write_reg(RDMA_RING_CFG, 1 << DESC_INDEX);
    }

    /// Enable DMA
    /// Source: U-Boot bcmgenet.c::bcmgenet_enable_dma()
    fn enable_dma(&mut self) {
        let dma_ctrl = DMA_CTRL_EN | (1 << (DESC_INDEX + DMA_RING_BUF_EN_SHIFT));

        self.write_reg(TDMA_SCB_BURST_SIZE, DMA_MAX_BURST_LENGTH);
        self.write_reg(RDMA_SCB_BURST_SIZE, DMA_MAX_BURST_LENGTH);

        self.write_reg(TDMA_CTRL, dma_ctrl);

        let rdma_ctrl = self.read_reg(RDMA_CTRL);
        self.write_reg(RDMA_CTRL, rdma_ctrl | dma_ctrl);
    }

    /// Disable DMA
    fn disable_dma(&mut self) {
        let tdma_ctrl = self.read_reg(TDMA_CTRL);
        self.write_reg(TDMA_CTRL, tdma_ctrl & !DMA_CTRL_EN);

        let rdma_ctrl = self.read_reg(RDMA_CTRL);
        self.write_reg(RDMA_CTRL, rdma_ctrl & !DMA_CTRL_EN);
    }

    /// Enable Hardware Filter Block (HFB) for ARP-only filtering
    ///
    /// Configures the GENET HFB to filter packets in hardware before they reach
    /// RX descriptors. This prevents interrupt storms on busy networks by only
    /// allowing ARP packets (EtherType 0x0806) to trigger interrupts.
    ///
    /// For full multi-protocol filtering implementation, see TODO-HFB.md.
    ///
    /// Source: Linux kernel drivers/net/ethernet/broadcom/genet/bcmgenet.c
    ///         Function: bcmgenet_hfb_create_rxnfc_filter()
    fn enable_arp_filter(&mut self) {
        // Filter 0: Match EtherType 0x0806 (ARP) at offset 12-13 in Ethernet frame
        let filter_id = 0;
        let filter_addr = GENET_HFB_OFF + HFB_FLT_BASE + (filter_id * 32); // 32 bytes per filter

        // Pattern: offset=12 (EtherType field), data=0x0806 (ARP), mask=0xFFFF (match all bits)
        // Ethernet frame: [0-5: dest MAC][6-11: src MAC][12-13: EtherType][14+: payload]
        self.write_reg(filter_addr + 0, 12); // Offset to match (EtherType field)
        self.write_reg(filter_addr + 4, 0x0806); // ARP EtherType (big-endian)
        self.write_reg(filter_addr + 8, 0xFFFF); // Mask (match all bits)
        self.write_reg(filter_addr + 12, 0); // Reserved/unused

        // Set filter length (16 bytes = check through EtherType)
        // Length registers hold 4 filter lengths each (8 bits per filter)
        let len_reg = GENET_HFB_OFF + HFB_FLT_LEN_V3PLUS[filter_id / 4];
        let shift = (filter_id % 4) * 8;
        let len_val = self.read_reg(len_reg);
        self.write_reg(len_reg, (len_val & !(0xFF << shift)) | (16 << shift));

        // Enable filter 0 (bit 0 in 32-bit enable mask)
        self.write_reg(GENET_HFB_OFF + HFB_FLT_ENABLE_V3PLUS, 1 << filter_id);

        // Set action: route matched packets to RX ring 16 (our default ring)
        // Lower 8 bits = ring number (0x10 = ring 16)
        self.write_reg(GENET_HFB_OFF + HFB_RXNFC_LKUP_CTRL + (filter_id * 4), 0x10);

        // Enable HFB globally (bit 0 = enable)
        self.write_reg(GENET_HFB_OFF + HFB_CTRL, 0x01);

        // Verify programming by reading back registers
        let ctrl = self.read_reg(GENET_HFB_OFF + HFB_CTRL);
        let enable_mask = self.read_reg(GENET_HFB_OFF + HFB_FLT_ENABLE_V3PLUS);
        let len_val = self.read_reg(len_reg);
        let offset = self.read_reg(filter_addr + 0);
        let ethertype = self.read_reg(filter_addr + 4);
        let mask = self.read_reg(filter_addr + 8);

        println!("[GENET] HFB filter enabled: ARP-only (EtherType 0x0806)");
        println!("[GENET]   HFB_CTRL: 0x{:08X} (expect 0x00000001)", ctrl);
        println!(
            "[GENET]   Filter enable mask: 0x{:08X} (expect 0x00000001)",
            enable_mask
        );
        println!(
            "[GENET]   Filter length: 0x{:08X} (expect 0x00000010 in bits [7:0])",
            len_val
        );
        println!(
            "[GENET]   Filter pattern: offset={}, ethertype=0x{:04X}, mask=0x{:04X}",
            offset, ethertype, mask
        );
    }

    /// Main initialization sequence
    ///
    /// Queries the MAC address from VideoCore firmware (stored in OTP during manufacturing)
    /// and configures the GENET controller.
    ///
    /// Source: U-Boot bcmgenet.c::bcmgenet_gmac_eth_start()
    /// Initialize GENET hardware
    ///
    /// # Arguments
    /// * `mailbox_addr` - Optional mailbox base address from device tree.
    ///   If None, uses hardcoded MAILBOX_BASE constant.
    pub fn initialize(&mut self, mailbox_addr: Option<usize>) -> Result<(), NetworkError> {
        if !self.is_present() {
            return Err(NetworkError::HardwareNotPresent);
        }

        println!("[GENET] Initializing GENET v5...");

        // Query MAC address from VideoCore firmware (reads from OTP)
        // Source: U-Boot board/raspberrypi/rpi/rpi.c::set_usbethaddr()
        use crate::drivers::mailbox::PropertyMailbox;
        let mailbox = if let Some(addr) = mailbox_addr {
            PropertyMailbox::with_base_addr(addr)
        } else {
            PropertyMailbox::new()
        };

        let mac_bytes = mailbox
            .get_mac_address()
            .map_err(|_| NetworkError::HardwareNotPresent)?;
        let mac = MacAddress::new(mac_bytes);
        println!("[GENET] MAC address from OTP: {}", mac);

        // Disable all interrupts (GIC level)
        {
            use crate::drivers::irqchip::gic_v2::GIC;
            use crate::drivers::irqchip::gic_v2::irq::{GENET_0, GENET_1};
            let gic = GIC.lock();
            gic.disable_interrupt(GENET_0);
            gic.disable_interrupt(GENET_1);
        }

        // Mask all GENET-internal interrupts
        self.write_reg(INTRL2_0_OFF + INTRL2_CPU_MASK_SET, 0xFFFFFFFF);
        self.write_reg(INTRL2_1_OFF + INTRL2_CPU_MASK_SET, 0xFFFFFFFF);
        self.write_reg(INTRL2_0_OFF + INTRL2_CPU_CLEAR, 0xFFFFFFFF);
        self.write_reg(INTRL2_1_OFF + INTRL2_CPU_CLEAR, 0xFFFFFFFF);

        // Power up EXT block
        self.write_reg(SYS_PORT_CTRL, PORT_MODE_EXT_GPHY);
        SystemTimer::delay_us(10);

        let mut pwr_mgmt = self.read_reg(EXT_PWR_MGMT);
        pwr_mgmt &=
            !(EXT_PWR_DOWN_DLL | EXT_PWR_DOWN_BIAS | EXT_ENERGY_DET_MASK | EXT_PWR_DOWN_PHY);
        self.write_reg(EXT_PWR_MGMT, pwr_mgmt);
        SystemTimer::delay_us(10);

        // Configure RGMII
        let rgmii = self.read_reg(EXT_RGMII_OOB_CTRL);
        let rgmii = rgmii
            | EXT_RGMII_OOB_ID_MODE_DISABLE
            | EXT_RGMII_OOB_RGMII_MODE_EN
            | EXT_RGMII_OOB_OOB_DISABLE;
        self.write_reg(EXT_RGMII_OOB_CTRL, rgmii);

        // Reset UMAC (this clears MAC registers)
        self.umac_reset();

        // Reprogram MAC address after reset
        self.mac_address = mac;
        let bytes = mac.as_bytes();
        let mac0 = (bytes[0] as u32) << 24
            | (bytes[1] as u32) << 16
            | (bytes[2] as u32) << 8
            | bytes[3] as u32;
        let mac1 = (bytes[4] as u32) << 8 | bytes[5] as u32;
        self.write_reg(UMAC_MAC0, mac0);
        self.write_reg(UMAC_MAC1, mac1);

        // Disable DMA before ring setup
        self.disable_dma();

        // Initialize rings
        self.rx_ring_init();
        self.rx_descs_init();
        self.tx_ring_init();

        // Enable DMA
        self.enable_dma();

        // Wait for link
        println!("[GENET] Waiting for link...");
        let mut link_up = false;
        for _ in 0..50 {
            if let Some(bmsr) = self.mdio_read(PHY_ADDR, MII_BMSR)
                && (bmsr & BMSR_LSTATUS) != 0
                && (bmsr & BMSR_ANEGCOMPLETE) != 0
            {
                link_up = true;
                break;
            }
            SystemTimer::delay_ms(100);
        }

        if !link_up {
            println!("[GENET] Warning: Link not up");
        }

        // Configure speed/duplex
        let params = self.read_link_params().unwrap_or(LinkParams {
            speed: LinkSpeed::Speed1000,
            duplex: DuplexMode::Full,
        });

        let cmd_speed = match params.speed {
            LinkSpeed::Speed10 => CMD_SPEED_10,
            LinkSpeed::Speed100 => CMD_SPEED_100,
            LinkSpeed::Speed1000 => CMD_SPEED_1000,
        };

        self.write_reg(UMAC_CMD, cmd_speed);

        let mode = if matches!(params.duplex, DuplexMode::Full) {
            MODE_FULL_DUPLEX
        } else {
            0
        };
        self.write_reg(UMAC_MODE, mode);

        println!(
            "[GENET] Link: {} Mbps, {} duplex",
            match params.speed {
                LinkSpeed::Speed10 => 10,
                LinkSpeed::Speed100 => 100,
                LinkSpeed::Speed1000 => 1000,
            },
            if matches!(params.duplex, DuplexMode::Full) {
                "full"
            } else {
                "half"
            }
        );

        println!("[GENET] Configuring interrupts...");

        // Enable GENET RX interrupts before enabling TX/RX
        // Check for any pending interrupts first
        let pending_before = self.read_reg(INTRL2_0_OFF + INTRL2_CPU_STAT);
        println!(
            "[GENET]   Pending interrupts before clear: 0x{:08X}",
            pending_before
        );

        // Clear any pending interrupts first
        println!("[GENET]   Clearing pending interrupts");
        self.write_reg(INTRL2_0_OFF + INTRL2_CPU_CLEAR, 0xFFFFFFFF);

        // Verify they were cleared
        let pending_after = self.read_reg(INTRL2_0_OFF + INTRL2_CPU_STAT);
        println!(
            "[GENET]   Pending interrupts after clear: 0x{:08X}",
            pending_after
        );

        // Unmask ONLY RX DMA interrupt (bit 13), keep all others masked
        // First: Ensure all interrupts are masked (redundant but explicit)
        self.write_reg(INTRL2_0_OFF + INTRL2_CPU_MASK_SET, 0xFFFFFFFF);

        // Then: Unmask ONLY bit 13 (RX DMA)
        println!("[GENET]   Unmasking RX DMA interrupt (bit 13)");
        self.write_reg(INTRL2_0_OFF + INTRL2_CPU_MASK_CLEAR, UMAC_IRQ_RXDMA_MBDONE);

        // Verify interrupt mask (should be 0xFFFFDFFF - all masked except bit 13)
        let mask = self.read_reg(INTRL2_0_OFF + INTRL2_CPU_MASK_STATUS);
        println!(
            "[GENET]   Interrupt mask: 0x{:08X} (expect 0xFFFFDFFF)",
            mask
        );

        // Check RX ring state
        let rx_prod = self.read_reg(RDMA_RING16_PROD_INDEX);
        let rx_cons = self.read_reg(RDMA_RING16_CONS_INDEX);
        println!(
            "[GENET]   RX ring: PROD={} CONS={} (available: {})",
            rx_prod,
            rx_cons,
            (rx_prod.wrapping_sub(rx_cons)) & 0xFFFF
        );

        println!("[GENET] GENET-internal interrupts configured (waiting for GIC enable)");

        // TODO: Enable Hardware Filter Block to prevent interrupt storms from unwanted traffic
        // Currently disabled - implementation needs fixing (complex Linux encoding scheme)
        // See TODO-HFB.md for full implementation plan
        // self.enable_arp_filter();

        // Enable TX and RX
        println!("[GENET] Enabling TX/RX...");
        let mut cmd = self.read_reg(UMAC_CMD);
        println!("[GENET]   UMAC_CMD before: 0x{:08X}", cmd);
        cmd |= CMD_TX_EN | CMD_RX_EN;
        self.write_reg(UMAC_CMD, cmd);
        let cmd_after = self.read_reg(UMAC_CMD);
        println!("[GENET]   UMAC_CMD after: 0x{:08X}", cmd_after);

        self.initialized = true;
        println!("[GENET] Initialization complete");
        Ok(())
    }

    // ========================================================================
    // Debug/Statistics Functions
    // ========================================================================

    /// Read DMA ring indices for diagnostic purposes
    pub fn read_dma_indices(&self) -> (u32, u32) {
        let prod = self.read_reg(TDMA_RING16_PROD_INDEX);
        let cons = self.read_reg(TDMA_RING16_CONS_INDEX);
        (prod, cons)
    }

    /// Read MIB packet statistics
    pub fn read_stats(&self) -> PacketStats {
        PacketStats {
            tx_packets: self.read_reg(MIB_TX_GOOD_PKTS),
            tx_bytes: self.read_reg(MIB_TX_GOOD_OCTETS),
            tx_broadcast: self.read_reg(MIB_TX_BCAST_PKTS),
            tx_multicast: self.read_reg(MIB_TX_MCAST_PKTS),
            rx_packets: self.read_reg(MIB_RX_GOOD_PKTS),
            rx_bytes: self.read_reg(MIB_RX_GOOD_OCTETS),
            rx_unicast: self.read_reg(MIB_RX_UCAST_PKTS),
            rx_broadcast: self.read_reg(MIB_RX_BCAST_PKTS),
            rx_multicast: self.read_reg(MIB_RX_MCAST_PKTS),
            rx_fcs_errors: self.read_reg(MIB_RX_FCS_ERR),
            rx_align_errors: self.read_reg(MIB_RX_ALIGN_ERR),
        }
    }

    // ========================================================================
    // Transmit/Receive
    // ========================================================================

    fn transmit_frame(&mut self, frame: &[u8]) -> Result<(), NetworkError> {
        if !self.initialized {
            return Err(NetworkError::NotInitialized);
        }

        if frame.len() < MIN_FRAME_SIZE || frame.len() > MAX_FRAME_SIZE {
            return Err(NetworkError::FrameTooLarge);
        }

        // Copy to TX buffer
        self.tx_buffer[..frame.len()].copy_from_slice(frame);

        let buffer_addr = self.tx_buffer.as_ptr() as usize;

        // Flush cache to ensure DMA can see TX buffer contents
        self.cache_flush(buffer_addr, frame.len());

        // Write descriptor
        // Source: U-Boot writes direct pointer, NO DMA_BUS_OFFSET
        let buffer_phys_addr = buffer_addr as u32;
        let desc_offset = TDMA_DESC_OFF + (self.tx_index * DMA_DESC_SIZE);

        self.write_reg(desc_offset + DMA_DESC_ADDRESS_LO, buffer_phys_addr);
        self.write_reg(desc_offset + DMA_DESC_ADDRESS_HI, 0);

        let len_status = ((frame.len() as u32) << DMA_BUFLENGTH_SHIFT)
            | (0x3F << DMA_TX_QTAG_SHIFT)
            | DMA_TX_APPEND_CRC
            | DMA_SOP
            | DMA_EOP;
        self.write_reg(desc_offset + DMA_DESC_LENGTH_STATUS, len_status);

        // Advance TX index
        self.tx_index = (self.tx_index + 1) % RING_SIZE;

        // Increment and write producer index to trigger DMA
        // This tells hardware to transmit the packet
        let mut prod_index = self.read_reg(TDMA_RING16_PROD_INDEX);
        prod_index = prod_index.wrapping_add(1);
        self.write_reg(TDMA_RING16_PROD_INDEX, prod_index);

        // Non-blocking transmission (kernel pattern):
        // Return immediately after submitting to DMA ring. Hardware will transmit
        // asynchronously (typically completes in <100us for small frames at 1Gbps).
        //
        // This matches Linux kernel bcmgenet.c::bcmgenet_xmit() which returns
        // NETDEV_TX_OK immediately after ring submission.
        //
        // Note: We have a single TX buffer, so only one packet can be in-flight.
        // Calling transmit() again before the previous packet completes will
        // overwrite the buffer, but in practice TX is so fast this rarely happens.
        //
        // Future improvement (Milestone #15): Enable TX completion interrupts
        // (UMAC_IRQ_TXDMA_MBDONE) to properly track buffer state and support
        // multiple in-flight packets.
        Ok(())
    }

    fn receive_frame(&mut self) -> Option<&[u8]> {
        if !self.initialized {
            return None;
        }

        let prod_index = (self.read_reg(RDMA_RING16_PROD_INDEX) & 0xFFFF) as usize;

        // Check if new packet available
        if prod_index == self.rx_c_index {
            return None;
        }

        // Read descriptor
        let desc_offset = RDMA_DESC_OFF + (self.rx_index * DMA_DESC_SIZE);
        let length_status = self.read_reg(desc_offset + DMA_DESC_LENGTH_STATUS);

        let length = ((length_status >> DMA_BUFLENGTH_SHIFT) & DMA_BUFLENGTH_MASK) as usize;

        // Validate received frame length
        if !(MIN_FRAME_SIZE..=RX_BUF_LENGTH).contains(&length) {
            // Skip invalid frame
            self.rx_index = (self.rx_index + 1) % RING_SIZE;
            self.rx_c_index = (self.rx_c_index + 1) % RING_SIZE;
            self.write_reg(RDMA_RING16_CONS_INDEX, self.rx_c_index as u32);
            return None;
        }

        // Calculate buffer address and invalidate cache to see DMA writes
        let buffer_offset = self.rx_index * RX_BUF_LENGTH;
        // SAFETY: buffer_offset is guaranteed < RX_TOTAL_BUFSIZE by modulo operation on rx_index
        let buffer_start = unsafe { self.rxbuffer.as_ptr().add(buffer_offset) } as usize;

        // Invalidate cache to discard stale data and read DMA writes from DRAM
        self.cache_invalidate(buffer_start, length);

        // Return slice (skip 2-byte padding from RBUF_ALIGN_2B)
        let packet = &self.rxbuffer[buffer_offset + RX_BUF_OFFSET..buffer_offset + length];
        Some(packet)
    }

    fn free_rx_buffer(&mut self) {
        // Flush cache for this RX buffer before returning it to hardware
        // Source: U-Boot bcmgenet.c::bcmgenet_gmac_free_pkt()
        let buffer_offset = self.rx_index * RX_BUF_LENGTH;
        // SAFETY: buffer_offset is guaranteed < RX_TOTAL_BUFSIZE by modulo operation on rx_index
        let buffer_start = unsafe { self.rxbuffer.as_ptr().add(buffer_offset) } as usize;

        // Flush cache to ensure any CPU writes are visible to DMA
        self.cache_flush(buffer_start, RX_BUF_LENGTH);

        // Advance to next descriptor
        self.rx_index = (self.rx_index + 1) % RING_SIZE;
        self.rx_c_index = (self.rx_c_index + 1) % RING_SIZE;

        // Update hardware CONS_INDEX
        self.write_reg(RDMA_RING16_CONS_INDEX, self.rx_c_index as u32);
    }
}

// ============================================================================
// Interrupt Handler
// ============================================================================

/// Handle GENET RX interrupt (called from exception handler)
///
/// This function is called when RXDMA_MBDONE interrupt fires, indicating
/// that the hardware has received one or more packets.
///
/// # Deadlock Prevention
///
/// CRITICAL: This function MUST NOT call `println!` or any function that
/// acquires locks while holding `GENET.lock()`. The pattern is:
/// 1. Acquire GENET lock
/// 2. Drain RX ring and route packets
/// 3. Drop GENET lock
/// 4. Log results (safe - no lock held)
pub fn handle_interrupt() {
    use core::sync::atomic::{AtomicU32, Ordering};
    static DEBUG_COUNT: AtomicU32 = AtomicU32::new(0);
    let debug_num = DEBUG_COUNT.fetch_add(1, Ordering::Relaxed);

    let (packets_routed, packets_dropped) = {
        // CRITICAL: No println() while holding GENET lock - would deadlock with TX path
        let mut genet = GENET.lock();

        // Read interrupt status and mask (Linux kernel pattern)
        let int_status_raw = genet.read_reg(INTRL2_0_OFF + INTRL2_CPU_STAT);
        let int_mask = genet.read_reg(INTRL2_0_OFF + INTRL2_CPU_MASK_STATUS);

        // Filter to ONLY unmasked interrupts (mask bit: 1=masked, 0=unmasked)
        // Linux: status = read(STAT) & ~read(MASK_STATUS)
        let int_status = int_status_raw & !int_mask;

        // Clear the interrupts we're about to process
        genet.write_reg(INTRL2_0_OFF + INTRL2_CPU_CLEAR, int_status);

        // Check if this is an RX interrupt (bit 13 = RXDMA_MBDONE)
        if (int_status & UMAC_IRQ_RXDMA_MBDONE) == 0 {
            (0, 0) // Return tuple for closure
        } else {
            // Drain RX ring - route packets to sockets
            let mut routed = 0;
            let mut dropped = 0;
            let mut packet_count = 0;

            while let Some(frame_data) = genet.receive_frame() {
                // SAFETY: frame_data points into GENET's static DMA buffer
                // The packet pool will hold a reference to this data until all
                // sockets are done processing it.
                let frame_static: &'static [u8] = unsafe { core::mem::transmute(frame_data) };

                // Route packet to socket layer
                if unsafe { crate::net::router::route_packet(frame_static) } {
                    routed += 1;
                } else {
                    dropped += 1;
                }

                // CRITICAL: Free RX buffer so hardware can reuse descriptor
                genet.free_rx_buffer();

                packet_count += 1;

                // Safety valve: Limit max packets per interrupt to prevent starvation
                if packet_count >= 32 {
                    break; // Leave remaining packets for next interrupt
                }
            }

            (routed, dropped)
        } // End of else block for RX interrupt processing
    }; // Drop GENET lock BEFORE any println!

    // Log statistics outside lock (safe - no deadlock risk)
    if packets_routed > 0 || packets_dropped > 0 {
        static IRQ_COUNT: AtomicU32 = AtomicU32::new(0);
        let total = IRQ_COUNT.fetch_add(1, Ordering::Relaxed);

        // Rate-limited logging (first 10 interrupts only)
        if total < 10 {
            println!(
                "[GENET IRQ #{}] Routed {} pkts, dropped {}",
                total + 1,
                packets_routed,
                packets_dropped
            );
        }
    }
}

// ============================================================================
// NetworkDevice Trait Implementation
// ============================================================================

impl NetworkDevice for GenetController {
    fn is_present(&self) -> bool {
        self.is_present()
    }

    fn init(&mut self) -> Result<(), NetworkError> {
        // MAC address is read from hardware (firmware programs it from OTP)
        // Use hardcoded mailbox address (trait doesn't support passing DTB address)
        self.initialize(None)
    }

    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError> {
        self.transmit_frame(frame)
    }

    fn receive(&mut self) -> Option<&[u8]> {
        let frame = self.receive_frame()?;
        Some(frame)
    }

    fn mac_address(&self) -> MacAddress {
        self.mac_address
    }

    fn free_rx_buffer(&mut self) {
        self.free_rx_buffer();
    }
}

// ============================================================================
// Link Parameters and Statistics
// ============================================================================

#[derive(Debug, Clone, Copy)]
enum LinkSpeed {
    Speed10,
    Speed100,
    Speed1000,
}

#[derive(Debug, Clone, Copy)]
enum DuplexMode {
    Half,
    Full,
}

struct LinkParams {
    speed: LinkSpeed,
    duplex: DuplexMode,
}

/// Packet statistics from MIB counters
#[derive(Debug, Clone, Copy)]
pub struct PacketStats {
    pub tx_packets: u32,
    pub tx_bytes: u32,
    pub tx_broadcast: u32,
    pub tx_multicast: u32,
    pub rx_packets: u32,
    pub rx_bytes: u32,
    pub rx_unicast: u32,
    pub rx_broadcast: u32,
    pub rx_multicast: u32,
    pub rx_fcs_errors: u32,
    pub rx_align_errors: u32,
}

// ============================================================================
// Global GENET Instance
// ============================================================================

use crate::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    /// Global GENET controller instance
    ///
    /// Wrapped in a mutex to prevent data races. The mutex automatically
    /// disables interrupts while the lock is held, preventing deadlocks
    /// between the TX path and RX interrupt handler.
    ///
    /// Initialize with `GENET.lock().init()`.
    pub static ref GENET: Mutex<GenetController> = Mutex::new(GenetController::new());
}

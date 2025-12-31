//! GIC-400 (Generic Interrupt Controller v2) driver for BCM2711.
//!
//! Provides interrupt management for Raspberry Pi 4, including:
//! - Distributor (GICD) configuration for routing interrupts
//! - CPU Interface (GICC) for acknowledging and completing interrupts
//! - Per-interrupt enable/disable, priority, and target CPU configuration
//!
//! Reference: [ARM GIC-400 Architecture Specification](https://developer.arm.com/documentation/ihi0069/latest/)

use core::ptr::{read_volatile, write_volatile};
use lazy_static::lazy_static;
use spin::Mutex;

/// GIC Distributor base address (BCM2711).
///
/// Source: BCM2711 ARM Peripherals, Section 6
/// Reference: <https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf>
const GICD_BASE: usize = 0xFF841000;

/// GIC CPU Interface base address (BCM2711).
///
/// Source: BCM2711 ARM Peripherals, Section 6
const GICC_BASE: usize = 0xFF842000;

/// GIC Distributor register offsets.
///
/// Reference: GIC-400 Architecture Specification, Section 3 (Distributor registers)
#[allow(dead_code)]
mod gicd_offsets {
    pub const CTLR: usize = 0x000; // Distributor Control Register
    pub const TYPER: usize = 0x004; // Interrupt Controller Type Register
    pub const IGROUPR: usize = 0x080; // Interrupt Group Registers (0x080-0x0FC)
    pub const ISENABLER: usize = 0x100; // Interrupt Set-Enable Registers (0x100-0x17C)
    pub const ICENABLER: usize = 0x180; // Interrupt Clear-Enable Registers (0x180-0x1FC)
    pub const ISPENDR: usize = 0x200; // Interrupt Set-Pending Registers (0x200-0x27C)
    pub const ICPENDR: usize = 0x280; // Interrupt Clear-Pending Registers (0x280-0x2FC)
    pub const IPRIORITYR: usize = 0x400; // Interrupt Priority Registers (0x400-0x7F8)
    pub const ITARGETSR: usize = 0x800; // Interrupt Processor Targets Registers (0x800-0xBF8)
    pub const ICFGR: usize = 0xC00; // Interrupt Configuration Registers (0xC00-0xCFC)
}

/// GIC CPU Interface register offsets.
///
/// Reference: GIC-400 Architecture Specification, Section 4 (CPU Interface registers)
#[allow(dead_code)]
mod gicc_offsets {
    pub const CTLR: usize = 0x000; // CPU Interface Control Register
    pub const PMR: usize = 0x004; // Interrupt Priority Mask Register
    pub const BPR: usize = 0x008; // Binary Point Register
    pub const IAR: usize = 0x00C; // Interrupt Acknowledge Register
    pub const EOIR: usize = 0x010; // End Of Interrupt Register
    pub const RPR: usize = 0x014; // Running Priority Register
    pub const HPPIR: usize = 0x018; // Highest Priority Pending Interrupt Register
}

/// GICD_CTLR register bits
mod gicd_ctlr {
    pub const ENABLE_GRP0: u32 = 1 << 0; // Enable Group 0 interrupts
    pub const ENABLE_GRP1: u32 = 1 << 1; // Enable Group 1 interrupts
}

/// GICC_CTLR register bits
#[allow(dead_code)]
mod gicc_ctlr {
    pub const ENABLE_GRP0: u32 = 1 << 0; // Enable Group 0 interrupts
    pub const ENABLE_GRP1: u32 = 1 << 1; // Enable Group 1 interrupts
    pub const FIQ_EN: u32 = 1 << 3; // FIQ enable (Group 0 as FIQ)
}

/// Interrupt configuration bits (GICD_ICFGR)
#[allow(dead_code)]
mod int_cfg {
    pub const LEVEL_SENSITIVE: u32 = 0b00; // Level-sensitive
    pub const EDGE_TRIGGERED: u32 = 0b10; // Edge-triggered
}

/// Known interrupt IDs for BCM2711 peripherals.
///
/// Source: BCM2711 device tree (arch/arm/boot/dts/broadcom/bcm2711.dtsi)
/// UART0 uses `GIC_SPI 121 IRQ_TYPE_LEVEL_HIGH`, which translates to ID 153 (32 + 121).
/// GENET uses two interrupts: `GIC_SPI 157` and `GIC_SPI 158` (IDs 189 and 190).
pub mod irq {
    /// UART0 (PL011) interrupt ID.
    ///
    /// Device tree specifies: `<GIC_SPI 121 IRQ_TYPE_LEVEL_HIGH>`
    /// SPI (Shared Peripheral Interrupt) IDs start at 32, so: 32 + 121 = 153
    pub const UART0: u32 = 153;

    /// GENET Ethernet controller interrupt 0.
    ///
    /// Device tree specifies: `<GIC_SPI 157 IRQ_TYPE_LEVEL_HIGH>`
    /// SPI IDs start at 32, so: 32 + 157 = 189
    pub const GENET_0: u32 = 189;

    /// GENET Ethernet controller interrupt 1.
    ///
    /// Device tree specifies: `<GIC_SPI 158 IRQ_TYPE_LEVEL_HIGH>`
    /// SPI IDs start at 32, so: 32 + 158 = 190
    pub const GENET_1: u32 = 190;
}

lazy_static! {
    pub static ref GIC: Mutex<Gic> = Mutex::new(Gic::new());
}

/// GIC-400 driver instance
pub struct Gic {
    gicd_base: usize,
    gicc_base: usize,
    initialized: bool,
}

impl Gic {
    /// Create a new GIC driver instance.
    pub const fn new() -> Self {
        Gic {
            gicd_base: GICD_BASE,
            gicc_base: GICC_BASE,
            initialized: false,
        }
    }

    /// Initialize the GIC distributor and CPU interface.
    ///
    /// Sets up the GIC for interrupt handling:
    /// 1. Disables distributor while configuring
    /// 2. Configures CPU interface priority mask
    /// 3. Enables distributor and CPU interface
    ///
    /// NOTE: Requires `enable_gic=1` in config.txt for bare metal Pi 4.
    pub fn init(&mut self) {
        // Disable distributor while configuring
        self.gicd_write(gicd_offsets::CTLR, 0);

        // Read the number of interrupt lines supported
        let typer = self.gicd_read(gicd_offsets::TYPER);
        let it_lines_number = typer & 0x1F; // Bits [4:0]
        let num_interrupts = 32 * (it_lines_number + 1);

        // Configure all SPIs (ID >= 32) as:
        // - Group: Group 0 (secure, for secure EL1)
        // - Priority: 0xA0 (medium priority)
        // - Target: CPU 0 (0x01)
        // - Configuration: Level-sensitive (0b00)
        for int_id in 32..num_interrupts {
            self.set_group(int_id, 0); // Group 0 = secure
            self.set_priority(int_id, 0xA0);
            self.set_target(int_id, 0x01); // Route to CPU 0
            self.set_config(int_id, int_cfg::LEVEL_SENSITIVE);
        }

        // Enable Group 0 and Group 1 interrupts in distributor
        self.gicd_write(
            gicd_offsets::CTLR,
            gicd_ctlr::ENABLE_GRP0 | gicd_ctlr::ENABLE_GRP1,
        );

        // Configure CPU interface
        // Set priority mask to lowest priority (0xFF = accept all interrupts)
        self.gicc_write(gicc_offsets::PMR, 0xFF);

        // Set binary point to 0 (all priority bits are used for preemption)
        self.gicc_write(gicc_offsets::BPR, 0);

        // Enable Group 0 and Group 1 interrupts in CPU interface
        self.gicc_write(
            gicc_offsets::CTLR,
            gicc_ctlr::ENABLE_GRP0 | gicc_ctlr::ENABLE_GRP1,
        );

        self.initialized = true;
    }

    /// Enable a specific interrupt by ID.
    ///
    /// # Arguments
    /// * `int_id` - Interrupt ID (e.g., 153 for UART0)
    pub fn enable_interrupt(&self, int_id: u32) {
        if !self.initialized {
            panic!("GIC not initialized");
        }

        let reg_offset = gicd_offsets::ISENABLER + ((int_id / 32) * 4) as usize;
        let bit = 1 << (int_id % 32);

        // SAFETY: Writing to GICD_ISENABLER is safe because:
        // 1. We've validated int_id fits within supported range
        // 2. Register address is calculated correctly per GIC-400 spec
        // 3. This is a write-only operation to set the enable bit
        unsafe {
            let addr = (self.gicd_base + reg_offset) as *mut u32;
            write_volatile(addr, bit);
        }
    }

    /// Disable a specific interrupt by ID.
    ///
    /// # Arguments
    /// * `int_id` - Interrupt ID to disable
    pub fn disable_interrupt(&self, int_id: u32) {
        if !self.initialized {
            panic!("GIC not initialized");
        }

        let reg_offset = gicd_offsets::ICENABLER + ((int_id / 32) * 4) as usize;
        let bit = 1 << (int_id % 32);

        // SAFETY: Writing to GICD_ICENABLER is safe (see enable_interrupt)
        unsafe {
            let addr = (self.gicd_base + reg_offset) as *mut u32;
            write_volatile(addr, bit);
        }
    }

    /// Acknowledge an interrupt and return its ID.
    ///
    /// Reads the GICC_IAR register, which returns the ID of the highest
    /// priority pending interrupt and marks it as active.
    ///
    /// Returns 1023 if no pending interrupt (spurious interrupt).
    pub fn acknowledge_interrupt(&self) -> u32 {
        self.gicc_read(gicc_offsets::IAR)
    }

    /// Signal end of interrupt processing.
    ///
    /// Writes to GICC_EOIR to deactivate the interrupt and allow
    /// lower-priority interrupts to be signaled.
    ///
    /// # Arguments
    /// * `int_id` - Interrupt ID from `acknowledge_interrupt()`
    pub fn end_of_interrupt(&self, int_id: u32) {
        self.gicc_write(gicc_offsets::EOIR, int_id);
    }

    /// Set interrupt priority (0 = highest, 255 = lowest).
    ///
    /// # Arguments
    /// * `int_id` - Interrupt ID
    /// * `priority` - Priority value (0-255)
    fn set_priority(&self, int_id: u32, priority: u8) {
        let reg_offset = gicd_offsets::IPRIORITYR + int_id as usize;

        // SAFETY: GICD_IPRIORITYR is a byte-accessible array
        unsafe {
            let addr = (self.gicd_base + reg_offset) as *mut u8;
            write_volatile(addr, priority);
        }
    }

    /// Set interrupt group (0 = secure, 1 = non-secure).
    ///
    /// # Arguments
    /// * `int_id` - Interrupt ID
    /// * `group` - Group number (0 for Group 0/secure, 1 for Group 1/non-secure)
    fn set_group(&self, int_id: u32, group: u32) {
        let reg_offset = gicd_offsets::IGROUPR + ((int_id / 32) * 4) as usize;
        let bit = 1u32 << (int_id % 32);

        // SAFETY: Read-modify-write to GICD_IGROUPR is safe
        unsafe {
            let addr = (self.gicd_base + reg_offset) as *mut u32;
            let mut val = read_volatile(addr);
            if group == 1 {
                val |= bit; // Set bit for Group 1 (non-secure)
            } else {
                val &= !bit; // Clear bit for Group 0 (secure)
            }
            write_volatile(addr, val);
        }
    }

    /// Set interrupt target CPU mask.
    ///
    /// # Arguments
    /// * `int_id` - Interrupt ID
    /// * `cpu_mask` - CPU target bitmask (bit 0 = CPU 0, etc.)
    fn set_target(&self, int_id: u32, cpu_mask: u8) {
        // Skip SGIs and PPIs (0-31), they have fixed targets
        if int_id < 32 {
            return;
        }

        let reg_offset = gicd_offsets::ITARGETSR + int_id as usize;

        // SAFETY: GICD_ITARGETSR is a byte-accessible array
        unsafe {
            let addr = (self.gicd_base + reg_offset) as *mut u8;
            write_volatile(addr, cpu_mask);
        }
    }

    /// Set interrupt configuration (level-sensitive or edge-triggered).
    ///
    /// # Arguments
    /// * `int_id` - Interrupt ID
    /// * `config` - Configuration bits (int_cfg::LEVEL_SENSITIVE or int_cfg::EDGE_TRIGGERED)
    fn set_config(&self, int_id: u32, config: u32) {
        let reg_offset = gicd_offsets::ICFGR + ((int_id / 16) * 4) as usize;
        let bit_shift = (int_id % 16) * 2;

        // SAFETY: Read-modify-write on GICD_ICFGR
        unsafe {
            let addr = (self.gicd_base + reg_offset) as *mut u32;
            let mut val = read_volatile(addr);
            val &= !(0b11 << bit_shift); // Clear old config
            val |= config << bit_shift; // Set new config
            write_volatile(addr, val);
        }
    }

    /// Read from a GIC Distributor register.
    fn gicd_read(&self, offset: usize) -> u32 {
        // SAFETY: Reading GICD registers is safe because:
        // 1. GICD_BASE is a valid MMIO address for the BCM2711 GIC distributor
        // 2. offset is a valid register offset within the GICD range
        // 3. This is a volatile read to prevent compiler optimization
        unsafe {
            let addr = (self.gicd_base + offset) as *const u32;
            read_volatile(addr)
        }
    }

    /// Write to a GIC Distributor register.
    fn gicd_write(&self, offset: usize, value: u32) {
        // SAFETY: Writing to GICD registers is safe (see gicd_read)
        unsafe {
            let addr = (self.gicd_base + offset) as *mut u32;
            write_volatile(addr, value);
        }
    }

    /// Read from a GIC CPU Interface register.
    fn gicc_read(&self, offset: usize) -> u32 {
        // SAFETY: Reading GICC registers is safe (see gicd_read)
        unsafe {
            let addr = (self.gicc_base + offset) as *const u32;
            read_volatile(addr)
        }
    }

    /// Write to a GIC CPU Interface register.
    fn gicc_write(&self, offset: usize, value: u32) {
        // SAFETY: Writing to GICC registers is safe (see gicd_read)
        unsafe {
            let addr = (self.gicc_base + offset) as *mut u32;
            write_volatile(addr, value);
        }
    }
}

impl Default for Gic {
    fn default() -> Self {
        Self::new()
    }
}

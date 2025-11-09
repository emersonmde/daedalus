//! ARMv8-A Memory Management Unit (MMU) implementation
//!
//! This module implements virtual memory support using ARMv8-A translation tables.
//!
//! ## Configuration
//! - **Address space**: 39-bit (512 GB virtual address space)
//! - **Page size**: 4 KB granule
//! - **Translation levels**: 3 levels (L1, L2, L3)
//! - **Mapping**: Identity mapping (VA = PA)
//!
//! ## Memory Layout
//! - Kernel region (0x00080000+): Normal memory, cacheable
//! - MMIO region (0xFE000000-0xFF800000): Device memory, non-cacheable
//!
//! ## References
//! - ARM ARM Section D4: The AArch64 Virtual Memory System Architecture
//! - Cortex-A72 TRM Section 8: Memory Management Unit

use core::arch::asm;
use core::ptr::{addr_of, addr_of_mut};

// ============================================================================
// Memory Attribute Constants
// ============================================================================

/// Memory Attribute Indirection Register (MAIR) indices
///
/// MAIR_EL1 defines 8 memory attribute encodings (0-7).
/// Each page table entry references one of these via AttrIndx\[2:0\] field.
///
/// Reference: ARM ARM Section D4.4.4
#[repr(u64)]
#[allow(dead_code)]
pub enum MemoryAttribute {
    /// Device memory - nGnRnE (non-Gathering, non-Reordering, no Early-ack)
    /// Used for MMIO registers where every access must be strictly ordered.
    Device = 0,

    /// Normal memory - Write-Back, Read/Write-Allocate cacheable
    /// Used for kernel code, data, and heap.
    Normal = 1,
}

/// MAIR_EL1 register value
///
/// Attr0 (Device): 0x00 = Device-nGnRnE
/// Attr1 (Normal): 0xFF = Normal, Inner/Outer Write-Back Read/Write-Allocate
///
/// Reference: ARM ARM Section D4.4.4, Table D4-17
const MAIR_VALUE: u64 = 0xFF << 8; // Attr1: Normal WB, Attr0: Device-nGnRnE (0x00)

// ============================================================================
// Translation Control Register (TCR_EL1) Configuration
// ============================================================================

/// TCR_EL1 register value for 4KB pages, 39-bit VA
///
/// Bit fields:
/// - T0SZ\[5:0\] = 25: 2^(64-25) = 2^39 = 512 GB address space
/// - T1SZ\[21:16\] = 25: Same for TTBR1_EL1 (we don't use it)
/// - TG0\[15:14\] = 0b00: 4KB granule for TTBR0_EL1
/// - TG1\[31:30\] = 0b10: 4KB granule for TTBR1_EL1
/// - SH0\[13:12\] = 0b11: Inner Shareable (for SMP)
/// - ORGN0\[11:10\] = 0b01: Normal, Outer Write-Back Write-Allocate Cacheable
/// - IRGN0\[9:8\] = 0b01: Normal, Inner Write-Back Write-Allocate Cacheable
/// - IPS\[34:32\] = 0b010: 40-bit physical address (1 TB, sufficient for Pi 4)
///
/// Reference: ARM ARM Section D4.2.6, Table D4-11
const TCR_VALUE: u64 = 25 // T0SZ = 25 (39-bit VA)
                     | (0b11 << 12)   // SH0 = Inner Shareable
                     | (0b01 << 10)   // ORGN0 = WB WA
                     | (0b01 << 8)    // IRGN0 = WB WA
                     | (25 << 16)     // T1SZ = 25 (we don't use TTBR1)
                     | (0b10 << 30)   // TG1 = 4KB
                     | (0b010 << 32); // IPS = 40-bit PA (1 TB)

// ============================================================================
// Page Table Entry Descriptor Formats
// ============================================================================

/// Descriptor type bits \[1:0\]
///
/// Reference: ARM ARM Section D4.3.1, Figure D4-7
const DESCRIPTOR_INVALID: u64 = 0b00;
const DESCRIPTOR_BLOCK: u64 = 0b01; // L1/L2: Block entry
const DESCRIPTOR_TABLE: u64 = 0b11; // L0/L1/L2: Table entry
const DESCRIPTOR_PAGE: u64 = 0b11; // L3: Page entry (same as table)

/// Access flag (bit 10): Must be set to avoid access faults
///
/// When AF = 1, hardware won't generate access flag faults.
///
/// Reference: ARM ARM Section D4.3.3
const DESCRIPTOR_AF: u64 = 1 << 10;

/// Shareability field \[9:8\]
///
/// Non-shareable = 0b00 (for device memory)
/// Inner Shareable = 0b11 (for normal memory, SMP)
///
/// Reference: ARM ARM Section D4.3.3
const DESCRIPTOR_SH_NON_SHAREABLE: u64 = 0b00 << 8;
const DESCRIPTOR_SH_INNER_SHAREABLE: u64 = 0b11 << 8;

/// Access permissions \[7:6\]
///
/// AP\[2:1\] encoding:
/// - 0b00: EL1 RW, EL0 no access (kernel only)
/// - 0b01: EL1/EL0 RW (kernel + user)
/// - 0b10: EL1 RO, EL0 no access (kernel read-only)
/// - 0b11: EL1/EL0 RO (kernel + user read-only)
///
/// Reference: ARM ARM Section D4.3.3, Table D4-18
const DESCRIPTOR_AP_EL1_RW: u64 = 0b00 << 6;

/// Non-secure bit (bit 5): Set for non-secure memory
const DESCRIPTOR_NS: u64 = 0 << 5; // We operate in non-secure world

/// Memory attribute index \[4:2\]
///
/// References MAIR_EL1 Attr0-Attr7
#[allow(dead_code)]
const fn attr_index(attr: MemoryAttribute) -> u64 {
    (attr as u64) << 2
}

/// Block/Page descriptor for device memory (MMIO)
///
/// - Descriptor type: Block (L1/L2) or Page (L3)
/// - Access flag: Set
/// - Shareability: Non-shareable (device memory shouldn't be shared)
/// - Access permission: EL1 RW only
/// - Memory attribute: Device-nGnRnE
const fn device_descriptor(addr: u64, level: usize) -> u64 {
    let desc_type = if level == 3 {
        DESCRIPTOR_PAGE
    } else {
        DESCRIPTOR_BLOCK
    };

    addr | desc_type
        | DESCRIPTOR_AF
        | DESCRIPTOR_SH_NON_SHAREABLE
        | DESCRIPTOR_AP_EL1_RW
        | DESCRIPTOR_NS
        | attr_index(MemoryAttribute::Device)
}

/// Block/Page descriptor for normal memory (kernel/DRAM)
///
/// - Descriptor type: Block (L1/L2) or Page (L3)
/// - Access flag: Set
/// - Shareability: Inner Shareable (for SMP)
/// - Access permission: EL1 RW only
/// - Memory attribute: Normal Write-Back
const fn normal_descriptor(addr: u64, level: usize) -> u64 {
    let desc_type = if level == 3 {
        DESCRIPTOR_PAGE
    } else {
        DESCRIPTOR_BLOCK
    };

    addr | desc_type
        | DESCRIPTOR_AF
        | DESCRIPTOR_SH_INNER_SHAREABLE
        | DESCRIPTOR_AP_EL1_RW
        | DESCRIPTOR_NS
        | attr_index(MemoryAttribute::Normal)
}

/// Table descriptor pointing to next-level table
///
/// - Descriptor type: Table
/// - Address: Physical address of next-level table (must be 4KB aligned)
///
/// Reference: ARM ARM Section D4.3.1
const fn table_descriptor(table_addr: u64) -> u64 {
    table_addr | DESCRIPTOR_TABLE
}

// ============================================================================
// Translation Table Structures
// ============================================================================

/// Number of entries in a translation table (4KB page / 8 bytes per entry)
const TABLE_ENTRIES: usize = 512;

/// A single translation table (4 KB, 512 entries × 8 bytes)
///
/// Must be aligned to 4 KB boundary.
#[repr(C)]
#[repr(align(4096))]
struct TranslationTable {
    entries: [u64; TABLE_ENTRIES],
}

impl TranslationTable {
    const fn new() -> Self {
        Self {
            entries: [DESCRIPTOR_INVALID; TABLE_ENTRIES],
        }
    }

    /// Set an entry in the translation table
    fn set_entry(&mut self, index: usize, value: u64) {
        self.entries[index] = value;
    }
}

// ============================================================================
// Static Page Tables
// ============================================================================

/// Level 1 translation table (covers 512 GB)
///
/// Each L1 entry covers 1 GB of address space.
static mut L1_TABLE: TranslationTable = TranslationTable::new();

/// Level 2 translation table for low memory (0-1 GB)
///
/// Each L2 entry covers 2 MB of address space.
/// We need this for the kernel region starting at 0x80000.
static mut L2_TABLE_LOW: TranslationTable = TranslationTable::new();

/// Level 2 translation table for MMIO region
///
/// MMIO starts at 0xFE000000, which falls in the 1 GB region
/// at index 0 (first 1 GB).
/// However, we'll use a separate L2 table for clarity when mapping MMIO.
static mut L2_TABLE_MMIO: TranslationTable = TranslationTable::new();

// ============================================================================
// MMU Initialization
// ============================================================================

/// Initialize the MMU and enable virtual memory
///
/// This function:
/// 1. Sets up translation tables for identity mapping
/// 2. Configures memory attributes (MAIR_EL1)
/// 3. Configures translation control (TCR_EL1)
/// 4. Sets translation table base (TTBR0_EL1)
/// 5. Enables the MMU (SCTLR_EL1.M)
///
/// # Safety
/// Must be called exactly once during boot, before any virtual memory access.
/// After this function returns, the MMU is enabled and all addresses are virtual
/// (but we use identity mapping so VA = PA).
pub unsafe fn init() {
    // SAFETY: This entire function is unsafe and called exactly once during boot
    unsafe {
        // Set up page tables
        setup_page_tables();

        // Configure memory attributes
        asm!("msr mair_el1, {}", in(reg) MAIR_VALUE, options(nomem, nostack));

        // Configure translation control
        asm!("msr tcr_el1, {}", in(reg) TCR_VALUE, options(nomem, nostack));

        // Set translation table base register (L1 table)
        // Use addr_of! to get pointer without creating reference
        let ttbr0 = addr_of!(L1_TABLE) as u64;
        asm!("msr ttbr0_el1, {}", in(reg) ttbr0, options(nomem, nostack));

        // Ensure all writes are complete before enabling MMU
        asm!("dsb sy", options(nostack));
        asm!("isb", options(nostack));

        // Enable MMU by setting M bit in SCTLR_EL1
        enable_mmu();
    }
}

/// Set up page tables for identity mapping
///
/// Maps:
/// - 0x00000000-0x3FFFFFFF (1 GB): Normal memory (kernel + DRAM)
/// - 0xFE000000-0xFFFFFFFF: Device memory (MMIO)
unsafe fn setup_page_tables() {
    // SAFETY: Called once during boot, single-threaded access to static mut
    unsafe {
        // L1 entry 0: Points to L2_TABLE_LOW (covers 0-1 GB)
        // Use addr_of_mut! to get pointers without creating references
        let l1_ptr = addr_of_mut!(L1_TABLE);
        let l2_low_addr = addr_of!(L2_TABLE_LOW) as u64;
        (*l1_ptr).set_entry(0, table_descriptor(l2_low_addr));

        // L2 entries for low memory (0-1 GB)
        // Each L2 entry is a 2 MB block
        // Map first 1 GB as normal memory for kernel + DRAM
        let l2_low_ptr = addr_of_mut!(L2_TABLE_LOW);
        for i in 0..TABLE_ENTRIES {
            let addr = (i * 2 * 1024 * 1024) as u64; // 2 MB blocks
            (*l2_low_ptr).set_entry(i, normal_descriptor(addr, 2));
        }

        // L1 entry for MMIO region
        // MMIO base is 0xFE000000 = 4064 MB
        // L1 index = 4064 MB / 1024 MB = 3.96... ≈ 3 (covers 3-4 GB region)
        // Actually, let's calculate precisely:
        // 0xFE000000 / (1 GB) = 0xFE000000 / 0x40000000 = 3.96875
        // So MMIO starts in L1 entry 3 (which covers 3-4 GB)

        // For simplicity, map the entire 3-4 GB region as device memory using L2 table
        let l2_mmio_addr = addr_of!(L2_TABLE_MMIO) as u64;
        (*l1_ptr).set_entry(3, table_descriptor(l2_mmio_addr));

        // L2 entries for MMIO region (3-4 GB, but we only need 0xFE000000-0xFF800000)
        // Each L2 entry is a 2 MB block starting at 3 GB
        let l2_mmio_ptr = addr_of_mut!(L2_TABLE_MMIO);
        for i in 0..TABLE_ENTRIES {
            let addr = (3 * 1024 * 1024 * 1024 + i * 2 * 1024 * 1024) as u64; // Start at 3 GB
            (*l2_mmio_ptr).set_entry(i, device_descriptor(addr, 2));
        }
    }
}

/// Enable the MMU by setting the M bit in SCTLR_EL1
///
/// Reference: ARM ARM Section D4.2.2
unsafe fn enable_mmu() {
    // SAFETY: Called once during MMU initialization
    unsafe {
        let mut sctlr: u64;

        // Read current SCTLR_EL1
        asm!("mrs {}, sctlr_el1", out(reg) sctlr, options(nomem, nostack));

        // Set required bits:
        // - M (bit 0): Enable MMU
        // - C (bit 2): Enable data cache
        // - I (bit 12): Enable instruction cache
        sctlr |= (1 << 0)  // M: MMU enable
               | (1 << 2)  // C: Data cache enable
               | (1 << 12); // I: Instruction cache enable

        // Write back SCTLR_EL1
        asm!("msr sctlr_el1, {}", in(reg) sctlr, options(nomem, nostack));

        // Synchronization barriers
        asm!("dsb sy", options(nostack));
        asm!("isb", options(nostack));
    }
}

/// Check if MMU is enabled
///
/// Returns true if SCTLR_EL1.M bit is set.
pub fn is_enabled() -> bool {
    let sctlr: u64;
    // SAFETY: Reading SCTLR_EL1 is safe - it's a read-only operation
    unsafe {
        asm!("mrs {}, sctlr_el1", out(reg) sctlr, options(nomem, nostack));
    }
    (sctlr & 1) != 0
}

/// Get the current translation table base address (TTBR0_EL1)
pub fn get_ttbr0() -> u64 {
    let ttbr0: u64;
    // SAFETY: Reading TTBR0_EL1 is safe - it's a read-only operation
    unsafe {
        asm!("mrs {}, ttbr0_el1", out(reg) ttbr0, options(nomem, nostack));
    }
    ttbr0
}

/// Get the Translation Control Register value
pub fn get_tcr() -> u64 {
    let tcr: u64;
    // SAFETY: Reading TCR_EL1 is safe - it's a read-only operation
    unsafe {
        asm!("mrs {}, tcr_el1", out(reg) tcr, options(nomem, nostack));
    }
    tcr
}

/// Get the Memory Attribute Indirection Register value
pub fn get_mair() -> u64 {
    let mair: u64;
    // SAFETY: Reading MAIR_EL1 is safe - it's a read-only operation
    unsafe {
        asm!("mrs {}, mair_el1", out(reg) mair, options(nomem, nostack));
    }
    mair
}

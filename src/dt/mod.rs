//! Device Tree Parsing
//!
//! Parses the Flattened Device Tree (FDT/DTB) passed by firmware to discover
//! hardware at runtime. This enables:
//! - Runtime hardware detection (QEMU vs real Pi 4 vs future Pi 5)
//! - Graceful handling of missing hardware (QEMU doesn't have GENET)
//! - Foundation for multi-board support
//!
//! # ARM Boot Protocol
//!
//! Per ARM boot protocol, firmware passes DTB address in x0 register:
//! 1. Firmware loads kernel at 0x80000
//! 2. Firmware passes DTB address in x0
//! 3. boot.s preserves x0 in x19 (callee-saved)
//! 4. boot.s passes DTB to _start_rust via x0
//! 5. Rust parses DTB for hardware discovery
//!
//! # Address Translation
//!
//! Device tree uses **bus addresses**, ARM CPU requires **physical addresses**:
//! - Bus: 0x7E200000 (GPIO in DT)  → Physical: 0xFE200000 (what CPU uses)
//! - Bus: 0x7D580000 (GENET in DT) → Physical: 0xFD580000 (what CPU uses)
//! - Bus: 0x40041000 (GIC in DT)   → Physical: 0xFF841000 (what CPU uses)
//!
//! Translation is done via `bus_to_physical()` using BCM2711-specific mappings.
//!
//! # Usage Pattern
//!
//! ```ignore
//! # #![no_std]
//! # use daedalus::dt::{HardwareInfo, bus_to_physical};
//! # use core::option::Option::{self, Some};
//! # use core::result::Result::{self, Ok};
//! # fn example() -> Result<(), &'static str> {
//! # let dtb_ptr: *const u8 = core::ptr::null();
//! // Parse DTB from firmware
//! let hw = HardwareInfo::from_firmware(dtb_ptr)?;
//!
//! // Find GENET ethernet controller
//! if let Some(genet) = hw.find_device("brcm,bcm2711-genet-v5") {
//!     let bus_addr = genet.base_address().unwrap();
//!     let phys_addr = bus_to_physical(bus_addr);
//!     // Initialize driver with phys_addr
//! } else {
//!     // GENET not present (QEMU), skip initialization
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - [Devicetree Specification](https://devicetree-specification.readthedocs.io/)
//! - [ARM Boot Protocol](https://www.kernel.org/doc/Documentation/arm64/booting.txt)
//! - [BCM2711 Device Tree](https://github.com/raspberrypi/linux/tree/rpi-6.6.y/arch/arm/boot/dts/broadcom)

use fdt_rs::base::DevTree;
use fdt_rs::prelude::*;

/// Hardware information parsed from device tree
///
/// Provides methods to query device tree for hardware components.
///
/// # Safety
///
/// Contains a raw pointer to DTB provided by firmware. The DTB must remain valid
/// for the lifetime of this struct. In practice, firmware-provided DTBs are static
/// and live for the entire kernel lifetime, making this safe.
///
/// This struct is intentionally NOT Send/Sync as the raw pointer cannot be safely
/// shared across threads without additional synchronization guarantees.
pub struct HardwareInfo {
    dtb_ptr: *const u8,
    dtb_size: usize,
}

impl HardwareInfo {
    /// Parse DTB from firmware-provided pointer
    ///
    /// Validates DTB magic number and parses the flattened device tree structure.
    ///
    /// # Safety
    ///
    /// `dtb_ptr` must point to a valid DTB blob provided by firmware.
    /// The DTB must remain valid for the lifetime of this HardwareInfo.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Magic number is not 0xd00dfeed
    /// - DTB structure is malformed
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn from_firmware(dtb_ptr: *const u8) -> Result<Self, &'static str> {
        // Check for null pointer (e.g., in tests)
        if dtb_ptr.is_null() {
            return Err("DTB pointer is null");
        }

        // SAFETY: Reading 4 bytes from dtb_ptr to check magic number.
        // Firmware guarantees DTB is valid and mapped.
        // Null check above ensures pointer is non-null.
        let magic = unsafe {
            u32::from_be_bytes([
                *dtb_ptr.add(0),
                *dtb_ptr.add(1),
                *dtb_ptr.add(2),
                *dtb_ptr.add(3),
            ])
        };

        if magic != 0xd00dfeed {
            return Err("Invalid DTB magic number");
        }

        // Read total size from DTB header
        // Create a temporary slice of the header (40 bytes is enough for FDT header)
        // SAFETY: dtb_ptr is valid (magic check passed), first 40 bytes contain header
        let size = unsafe {
            let header_slice = core::slice::from_raw_parts(dtb_ptr, 40);
            DevTree::read_totalsize(header_slice).map_err(|_| "Failed to read DTB size")?
        };

        Ok(Self {
            dtb_ptr,
            dtb_size: size,
        })
    }

    /// Get size of DTB in bytes
    pub fn size(&self) -> usize {
        self.dtb_size
    }

    /// Find a device by compatible string
    ///
    /// Searches device tree for a node with matching `compatible` property.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # #![no_std]
    /// # use daedalus::dt::HardwareInfo;
    /// # let hw = HardwareInfo::from_firmware(core::ptr::null()).unwrap();
    /// // Find GENET ethernet controller
    /// let genet = hw.find_device("brcm,bcm2711-genet-v5");
    ///
    /// // Find PL011 UART
    /// let uart = hw.find_device("arm,pl011");
    /// ```
    ///
    /// Returns `None` if no matching device found (e.g., GENET in QEMU).
    pub fn find_device(&self, compatible: &str) -> Option<DeviceNode> {
        // SAFETY: dtb_ptr and dtb_size are valid (verified in from_firmware)
        let dtb_bytes = unsafe { core::slice::from_raw_parts(self.dtb_ptr, self.dtb_size) };

        // SAFETY: dtb_bytes is valid DTB structure
        let dt = unsafe { DevTree::new(dtb_bytes).ok()? };

        // Find first node with matching compatible string
        let mut compatible_iter = dt.compatible_nodes(compatible);

        // Get first matching node
        // Iterator.next() returns Result<Option<Node>, Error>
        let node = match compatible_iter.next() {
            Ok(Some(n)) => n,
            _ => return None,
        };

        // Extract properties from the node
        let base_addr = Self::extract_base_address(&node);
        let interrupts = Self::extract_interrupts(&node);
        let clock_freq = Self::extract_clock_frequency(&node);

        Some(DeviceNode {
            name: node.name().unwrap_or(compatible).into(),
            base_addr,
            interrupts,
            clock_freq,
        })
    }

    /// Find a property by name in a node
    ///
    /// Helper to reduce code duplication in property extraction.
    fn find_property<'a, 'b, 'dt>(
        node: &'a fdt_rs::base::DevTreeNode<'b, 'dt>,
        name: &str,
    ) -> Option<fdt_rs::base::DevTreeProp<'a, 'dt>> {
        let mut props = node.props();
        while let Ok(Some(prop)) = props.next() {
            if prop.name() == Ok(name) {
                return Some(prop);
            }
        }
        None
    }

    /// Extract base address from "reg" property
    ///
    /// The "reg" property format varies by device tree configuration:
    /// - Most BCM2711 nodes use #address-cells=1, #size-cells=1 (32-bit)
    /// - Some nodes (like GENET) use #address-cells=2, #size-cells=2 (64-bit)
    /// - Format for 32-bit: `<addr size>`
    /// - Format for 64-bit: `<addr_high addr_low size_high size_low>`
    ///
    /// Returns the bus address (not physical).
    /// Caller must use `bus_to_physical()` to translate to ARM physical address.
    ///
    /// We try both formats and return the one that looks like a valid bus address.
    fn extract_base_address(node: &fdt_rs::base::DevTreeNode) -> Option<usize> {
        use fdt_rs::prelude::PropReader;

        let prop = Self::find_property(node, "reg")?;

        // Try reading as 64-bit address first (index 1 = addr_low)
        if let Ok(addr_64bit) = prop.u32(1) {
            // Check if this looks like a valid BCM2711 bus address
            let addr = addr_64bit as usize;
            // Valid ranges: 0x7C000000-0x7FFFFFFF, 0x40000000-0x407FFFFF
            if (0x7C000000..=0x7FFFFFFF).contains(&addr)
                || (0x40000000..=0x407FFFFF).contains(&addr)
            {
                return Some(addr);
            }
        }

        // Try reading as 32-bit address (index 0)
        if let Ok(addr_32bit) = prop.u32(0) {
            let addr = addr_32bit as usize;
            // Check if this looks like a valid bus address
            if (0x7C000000..=0x7FFFFFFF).contains(&addr)
                || (0x40000000..=0x407FFFFF).contains(&addr)
            {
                return Some(addr);
            }
        }

        None
    }

    /// Extract interrupt numbers from "interrupts" property
    ///
    /// Format for GIC: <type, number, flags> repeated
    /// - type: 0=SPI, 1=PPI
    /// - number: hardware IRQ number
    /// - flags: trigger type
    ///
    /// Returns all interrupt cells as u32 array
    fn extract_interrupts(node: &fdt_rs::base::DevTreeNode) -> alloc::vec::Vec<u32> {
        use fdt_rs::prelude::PropReader;

        let mut result = alloc::vec::Vec::new();

        if let Some(prop) = Self::find_property(node, "interrupts") {
            // Read all u32 values from property
            // PropReader::length() gives bytes, divide by 4 for u32 count
            let count = prop.length() / 4;
            for i in 0..count {
                if let Ok(val) = prop.u32(i) {
                    result.push(val);
                }
            }
        }

        result
    }

    /// Extract clock frequency from "clock-frequency" property
    ///
    /// Returns u32 value in Hz
    fn extract_clock_frequency(node: &fdt_rs::base::DevTreeNode) -> Option<u32> {
        use fdt_rs::prelude::PropReader;

        Self::find_property(node, "clock-frequency").and_then(|prop| prop.u32(0).ok())
    }
}

/// Represents a single device tree node
///
/// Stores extracted device properties (base address, interrupts, etc).
pub struct DeviceNode {
    name: alloc::string::String,
    base_addr: Option<usize>,
    interrupts: alloc::vec::Vec<u32>,
    clock_freq: Option<u32>,
}

impl DeviceNode {
    /// Get node name (e.g., "ethernet@7d580000")
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get base address from "reg" property
    ///
    /// Returns bus address - caller must use `bus_to_physical()` to translate
    /// to ARM physical address.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # #![no_std]
    /// # use daedalus::dt::{HardwareInfo, bus_to_physical};
    /// # let hw = HardwareInfo::from_firmware(core::ptr::null()).unwrap();
    /// # let node = hw.find_device("brcm,bcm2711-genet-v5").unwrap();
    /// let bus_addr = node.base_address().unwrap();        // 0x7d580000
    /// let phys_addr = bus_to_physical(bus_addr);  // 0xfd580000
    /// ```
    pub fn base_address(&self) -> Option<usize> {
        self.base_addr
    }

    /// Get interrupt numbers from "interrupts" property
    ///
    /// For GIC, interrupt format is: `<type, number, flags>`
    /// - type: 0=SPI, 1=PPI
    /// - number: Hardware IRQ number
    /// - flags: Trigger type (level/edge, active high/low)
    ///
    /// Returns raw interrupt cells - caller must decode based on interrupt controller.
    pub fn interrupts(&self) -> Option<&[u32]> {
        if self.interrupts.is_empty() {
            None
        } else {
            Some(&self.interrupts)
        }
    }

    /// Get u32 property value by name
    ///
    /// Currently only supports "clock-frequency" property.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # #![no_std]
    /// # use daedalus::dt::HardwareInfo;
    /// # let hw = HardwareInfo::from_firmware(core::ptr::null()).unwrap();
    /// # let node = hw.find_device("arm,pl011").unwrap();
    /// let clock_freq = node.property_u32("clock-frequency");  // Some(1000000) (1 MHz)
    /// ```
    pub fn property_u32(&self, name: &str) -> Option<u32> {
        match name {
            "clock-frequency" => self.clock_freq,
            _ => None,
        }
    }
}

/// Translate BCM2711 bus address to ARM physical address
///
/// Device tree uses bus addresses (from SoC perspective), but ARM CPU requires
/// physical addresses. This function translates based on BCM2711 memory map.
///
/// # Address Ranges
///
/// From BCM2711 device tree `ranges` property:
/// - 0x7E000000-0x7F7FFFFF → 0xFE000000-0xFFF7FFFF (BCM283x peripherals)
/// - 0x7C000000-0x7DFFFFFF → 0xFC000000-0xFDFFFFFF (BCM2711 peripherals)
/// - 0x40000000-0x407FFFFF → 0xFF800000-0xFFFFFFFF (ARM local/GIC)
///
/// # Example
///
/// ```ignore
/// assert_eq!(bus_to_physical(0x7E200000), 0xFE200000);  // GPIO
/// assert_eq!(bus_to_physical(0x7D580000), 0xFD580000);  // GENET
/// assert_eq!(bus_to_physical(0x40041000), 0xFF841000);  // GIC
/// ```
///
/// # Reference
///
/// Source: U-Boot dts/upstream/src/arm/broadcom/bcm2711.dtsi `ranges` property
pub fn bus_to_physical(bus_addr: usize) -> usize {
    match bus_addr {
        // BCM283x peripherals: 0x7E000000 → 0xFE000000
        // Add 0x80000000 offset
        addr if (0x7E000000..0x7F800000).contains(&addr) => addr + 0x80000000,

        // BCM2711 peripherals: 0x7C000000 → 0xFC000000
        // Add 0x80000000 offset
        addr if (0x7C000000..0x7E000000).contains(&addr) => addr + 0x80000000,

        // ARM local peripherals: 0x40000000 → 0xFF800000
        // Add 0xBF800000 offset
        addr if (0x40000000..0x40800000).contains(&addr) => addr + 0xBF800000,

        // Already physical or unknown - return as-is
        addr => addr,
    }
}

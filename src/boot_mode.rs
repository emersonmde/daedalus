//! Boot mode detection for bootstrap vs network kernel
//!
//! DaedalusOS supports two boot modes:
//! - **Bootstrap**: Loaded from SD card at 0x00080000, performs network fetch
//! - **Network**: Loaded from network at 0x01000000, runs full OS
//!
//! The same kernel binary can run in either mode. Mode is detected by examining
//! the current program counter to determine which memory region we're executing from.

use crate::arch::aarch64::kexec::layout;

/// Boot mode of the current kernel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    /// Bootstrap mode: Kernel loaded from SD card at 0x00080000
    ///
    /// In this mode, the kernel performs minimal initialization and then:
    /// 1. Initializes UART, networking, and basic subsystems
    /// 2. Fetches a new kernel from the development server via HTTP
    /// 3. Loads it to the staging area at 0x01000000
    /// 4. Performs kexec to jump to the new kernel
    Bootstrap,

    /// Network mode: Kernel loaded from network at 0x01000000
    ///
    /// In this mode, the kernel runs normally with full functionality:
    /// - Interactive shell
    /// - All drivers and subsystems
    /// - Network services
    /// - Watchdog for crash recovery
    Network,
}

impl BootMode {
    /// Detect boot mode by examining current program counter
    ///
    /// This uses the `adr` instruction to get the current PC and determines
    /// which memory region we're executing from.
    ///
    /// # Memory Layout
    /// - 0x00080000 - 0x04280000: Bootstrap kernel region (~66 MB)
    /// - 0x01000000 - 0x02000000: Network kernel region (16 MB)
    ///
    /// Note: These regions don't overlap. Bootstrap ends around 0x04280000,
    /// while network staging starts at 0x01000000.
    pub fn detect() -> Self {
        let pc = Self::read_pc();
        Self::from_address(pc)
    }

    /// Read current program counter
    ///
    /// Uses `adr` instruction which loads PC-relative address
    fn read_pc() -> usize {
        let pc: usize;
        // SAFETY: Reading the program counter is always safe. The `adr` instruction
        // loads the address of the current instruction into a register. We use inline
        // assembly because Rust doesn't provide a direct way to read PC.
        //
        // The `adr` instruction:
        // - Loads PC-relative address (safe, no memory access)
        // - Uses "." as the label (current location)
        // - nomem: doesn't access memory
        // - nostack: doesn't touch stack
        // - preserves_flags: doesn't modify condition flags
        unsafe {
            core::arch::asm!(
                "adr {}, .",
                out(reg) pc,
                options(nomem, nostack, preserves_flags)
            );
        }
        pc
    }

    /// Determine boot mode from a program counter address
    ///
    /// This allows testing without inline assembly
    pub fn from_address(addr: usize) -> Self {
        // Network kernel staging area: 0x01000000 - 0x02000000
        if (layout::NETWORK_KERNEL_BASE..layout::NETWORK_KERNEL_END).contains(&addr) {
            BootMode::Network
        } else {
            // Everything else is bootstrap (primary location is 0x00080000)
            BootMode::Bootstrap
        }
    }

    /// Returns true if this is bootstrap mode
    pub fn is_bootstrap(self) -> bool {
        matches!(self, BootMode::Bootstrap)
    }

    /// Returns true if this is network mode
    pub fn is_network(self) -> bool {
        matches!(self, BootMode::Network)
    }

    /// Get a human-readable description of the boot mode
    pub fn description(self) -> &'static str {
        match self {
            BootMode::Bootstrap => "Bootstrap (SD card)",
            BootMode::Network => "Network (remote loaded)",
        }
    }
}

impl core::fmt::Display for BootMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_bootstrap_mode_detection() {
        // Addresses in bootstrap region should be detected as Bootstrap
        assert_eq!(BootMode::from_address(0x00080000), BootMode::Bootstrap);
        assert_eq!(BootMode::from_address(0x00100000), BootMode::Bootstrap);
        assert_eq!(BootMode::from_address(0x04000000), BootMode::Bootstrap);
    }

    #[test_case]
    fn test_network_mode_detection() {
        // Addresses in network staging region should be detected as Network
        assert_eq!(BootMode::from_address(0x01000000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x01500000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x01FFFFFF), BootMode::Network);
    }

    #[test_case]
    fn test_boundary_conditions() {
        // Test boundaries between regions
        assert_eq!(BootMode::from_address(0x00FFFFFF), BootMode::Bootstrap);
        assert_eq!(BootMode::from_address(0x01000000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x01FFFFFF), BootMode::Network);
        assert_eq!(BootMode::from_address(0x02000000), BootMode::Bootstrap);
    }

    #[test_case]
    fn test_boot_mode_predicates() {
        assert!(BootMode::Bootstrap.is_bootstrap());
        assert!(!BootMode::Bootstrap.is_network());

        assert!(!BootMode::Network.is_bootstrap());
        assert!(BootMode::Network.is_network());
    }

    #[test_case]
    fn test_boot_mode_display() {
        use alloc::format;
        assert_eq!(format!("{}", BootMode::Bootstrap), "Bootstrap (SD card)");
        assert_eq!(format!("{}", BootMode::Network), "Network (remote loaded)");
    }
}

//! Boot mode detection (SD card vs network staging area)

use crate::arch::aarch64::kexec::layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    Bootstrap, // SD card at 0x00080000
    Network,   // Network staging (0x01000000 or 0x02000000)
}

impl BootMode {
    pub fn detect() -> Self {
        Self::from_address(Self::read_pc())
    }

    #[cfg(target_arch = "aarch64")]
    fn read_pc() -> usize {
        let pc: usize;
        unsafe {
            core::arch::asm!("adr {}, .", out(reg) pc, options(nomem, nostack, preserves_flags));
        }
        pc
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn read_pc() -> usize {
        layout::BOOTSTRAP_KERNEL_BASE
    }

    pub fn from_address(addr: usize) -> Self {
        let in_a = (layout::NETWORK_KERNEL_BASE_A..layout::NETWORK_KERNEL_END_A).contains(&addr);
        let in_b = (layout::NETWORK_KERNEL_BASE_B..layout::NETWORK_KERNEL_END_B).contains(&addr);
        if in_a || in_b {
            BootMode::Network
        } else {
            BootMode::Bootstrap
        }
    }

    pub fn is_bootstrap(self) -> bool {
        matches!(self, BootMode::Bootstrap)
    }

    pub fn is_network(self) -> bool {
        matches!(self, BootMode::Network)
    }

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
        // Addresses in network staging region A should be detected as Network
        assert_eq!(BootMode::from_address(0x01000000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x01500000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x01FFFFFF), BootMode::Network);

        // Addresses in network staging region B should be detected as Network
        assert_eq!(BootMode::from_address(0x02000000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x02500000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x02FFFFFF), BootMode::Network);
    }

    #[test_case]
    fn test_boundary_conditions() {
        // Test boundaries between regions
        assert_eq!(BootMode::from_address(0x00FFFFFF), BootMode::Bootstrap);

        // Staging area A: 0x01000000 - 0x02000000
        assert_eq!(BootMode::from_address(0x01000000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x01FFFFFF), BootMode::Network);

        // Staging area B: 0x02000000 - 0x03000000
        assert_eq!(BootMode::from_address(0x02000000), BootMode::Network);
        assert_eq!(BootMode::from_address(0x02FFFFFF), BootMode::Network);

        // After staging area B
        assert_eq!(BootMode::from_address(0x03000000), BootMode::Bootstrap);
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

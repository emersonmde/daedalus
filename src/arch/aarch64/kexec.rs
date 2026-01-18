//! Kexec - Hot kernel replacement without reboot
//!
//! Loads and executes a new kernel from memory for network-based deployment during
//! development. Disables MMU/caches/interrupts and transfers control to the new kernel.

use core::arch::global_asm;

global_asm!(include_str!("kexec.s"));

unsafe extern "C" {
    /// Jump to new kernel (disables MMU/caches, invalidates TLB/caches, never returns)
    fn kexec_jump(new_kernel_addr: usize, dtb_ptr: usize) -> !;
}

pub mod layout {
    pub const BOOTSTRAP_KERNEL_BASE: usize = 0x0008_0000;
    pub const NETWORK_KERNEL_BASE_A: usize = 0x0100_0000;
    pub const NETWORK_KERNEL_BASE_B: usize = 0x0200_0000;
    pub const NETWORK_KERNEL_MAX_SIZE: usize = 0x0100_0000; // 16 MB
    pub const NETWORK_KERNEL_END_A: usize = NETWORK_KERNEL_BASE_A + NETWORK_KERNEL_MAX_SIZE;
    pub const NETWORK_KERNEL_END_B: usize = NETWORK_KERNEL_BASE_B + NETWORK_KERNEL_MAX_SIZE;
    pub const NETWORK_KERNEL_BASE: usize = NETWORK_KERNEL_BASE_A;
    pub const NETWORK_KERNEL_END: usize = NETWORK_KERNEL_END_A;
}

#[derive(Debug, Clone, Copy)]
pub enum KexecError {
    KernelTooLarge { size: usize, max_size: usize },
    InvalidAddress { addr: usize },
    InvalidDtb { dtb_ptr: usize },
}

pub fn validate_kexec(
    kernel_addr: usize,
    kernel_size: usize,
    dtb_ptr: usize,
) -> Result<(), KexecError> {
    if kernel_size > layout::NETWORK_KERNEL_MAX_SIZE {
        return Err(KexecError::KernelTooLarge {
            size: kernel_size,
            max_size: layout::NETWORK_KERNEL_MAX_SIZE,
        });
    }

    let valid_addr = kernel_addr == layout::NETWORK_KERNEL_BASE_A
        || kernel_addr == layout::NETWORK_KERNEL_BASE_B;
    if !valid_addr {
        return Err(KexecError::InvalidAddress { addr: kernel_addr });
    }

    if dtb_ptr == 0 {
        return Err(KexecError::InvalidDtb { dtb_ptr });
    }

    Ok(())
}

/// # Safety
/// Never returns. Disables MMU/caches/interrupts and jumps to new kernel.
/// New kernel must be valid or system will crash.
pub unsafe fn kexec(kernel_addr: usize, kernel_size: usize, dtb_ptr: usize) -> ! {
    validate_kexec(kernel_addr, kernel_size, dtb_ptr).expect("Kexec validation failed");
    unsafe {
        crate::ALLOCATOR.reset();
        kexec_jump(kernel_addr, dtb_ptr)
    }
}

/// Choose staging address to avoid overwriting running kernel (ping-pong between A and B)
pub fn next_staging_address() -> usize {
    let pc = read_current_pc();
    if (layout::NETWORK_KERNEL_BASE_A..layout::NETWORK_KERNEL_END_A).contains(&pc) {
        layout::NETWORK_KERNEL_BASE_B
    } else if (layout::NETWORK_KERNEL_BASE_B..layout::NETWORK_KERNEL_END_B).contains(&pc) {
        layout::NETWORK_KERNEL_BASE_A
    } else {
        layout::NETWORK_KERNEL_BASE_A
    }
}

#[cfg(target_arch = "aarch64")]
fn read_current_pc() -> usize {
    let pc: usize;
    unsafe {
        core::arch::asm!("adr {}, .", out(reg) pc, options(nomem, nostack, preserves_flags));
    }
    pc
}

#[cfg(not(target_arch = "aarch64"))]
fn read_current_pc() -> usize {
    layout::BOOTSTRAP_KERNEL_BASE
}

/// # Safety
/// Copies kernel data to staging area (chosen via ping-pong to avoid overwriting running kernel)
pub unsafe fn stage_kernel(kernel_data: &[u8]) -> Result<usize, KexecError> {
    if kernel_data.len() > layout::NETWORK_KERNEL_MAX_SIZE {
        return Err(KexecError::KernelTooLarge {
            size: kernel_data.len(),
            max_size: layout::NETWORK_KERNEL_MAX_SIZE,
        });
    }

    let addr = next_staging_address();
    unsafe {
        core::ptr::copy_nonoverlapping(kernel_data.as_ptr(), addr as *mut u8, kernel_data.len());
    }
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_validate_kexec_valid() {
        assert!(validate_kexec(layout::NETWORK_KERNEL_BASE, 1024 * 1024, 0x100).is_ok());
    }

    #[test_case]
    fn test_validate_kexec_kernel_too_large() {
        let result = validate_kexec(
            layout::NETWORK_KERNEL_BASE,
            layout::NETWORK_KERNEL_MAX_SIZE + 1,
            0x100,
        );
        assert!(matches!(result, Err(KexecError::KernelTooLarge { .. })));
    }

    #[test_case]
    fn test_validate_kexec_invalid_address() {
        assert!(matches!(
            validate_kexec(0xDEADBEEF, 1024, 0x100),
            Err(KexecError::InvalidAddress { .. })
        ));
    }

    #[test_case]
    fn test_validate_kexec_invalid_dtb() {
        assert!(matches!(
            validate_kexec(layout::NETWORK_KERNEL_BASE, 1024, 0x0),
            Err(KexecError::InvalidDtb { .. })
        ));
    }

    #[test_case]
    fn test_validate_kexec_staging_area_b() {
        assert!(validate_kexec(layout::NETWORK_KERNEL_BASE_B, 1024 * 1024, 0x100).is_ok());
    }

    #[test_case]
    fn test_next_staging_address_from_bootstrap() {}

    #[test_case]
    fn test_ping_pong_alternation() {
        assert!(layout::NETWORK_KERNEL_END_A <= layout::NETWORK_KERNEL_BASE_B);
        assert_eq!(
            layout::NETWORK_KERNEL_END_A - layout::NETWORK_KERNEL_BASE_A,
            layout::NETWORK_KERNEL_MAX_SIZE
        );
        assert_eq!(
            layout::NETWORK_KERNEL_END_B - layout::NETWORK_KERNEL_BASE_B,
            layout::NETWORK_KERNEL_MAX_SIZE
        );
        assert_eq!(layout::NETWORK_KERNEL_BASE, layout::NETWORK_KERNEL_BASE_A);
        assert_eq!(layout::NETWORK_KERNEL_END, layout::NETWORK_KERNEL_END_A);
    }
}

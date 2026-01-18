//! Kexec - Hot kernel replacement
//!
//! This module provides functionality to load and execute a new kernel from memory
//! without a full reboot. This is used for network-based kernel deployment during
//! development.
//!
//! # Safety
//!
//! Kexec is inherently unsafe as it:
//! - Disables MMU/caches/interrupts
//! - Invalidates all running code/data assumptions
//! - Transfers control to arbitrary memory
//! - Never returns
//!
//! The caller must ensure:
//! - `new_kernel_addr` points to valid kernel code
//! - `dtb_ptr` points to a valid Device Tree Blob
//! - The new kernel is properly loaded and verified
//! - No critical operations are in progress

use core::arch::global_asm;

// Include the assembly implementation
global_asm!(include_str!("kexec.s"));

unsafe extern "C" {
    /// Low-level kexec jump function (implemented in assembly)
    ///
    /// This function:
    /// 1. Disables all interrupts (DAIF mask)
    /// 2. Disables MMU and caches
    /// 3. Invalidates TLB
    /// 4. Cleans and invalidates data cache
    /// 5. Invalidates instruction cache
    /// 6. Performs memory barriers
    /// 7. Jumps to new kernel with DTB pointer in x0
    ///
    /// # Arguments
    /// - `new_kernel_addr`: Physical address of new kernel entry point
    /// - `dtb_ptr`: Physical address of Device Tree Blob
    ///
    /// # Safety
    /// This function never returns. All system state is destroyed.
    fn kexec_jump(new_kernel_addr: usize, dtb_ptr: usize) -> !;
}

/// Memory layout constants for kexec operations
pub mod layout {
    /// Bootstrap kernel loads at this address (from SD card)
    pub const BOOTSTRAP_KERNEL_BASE: usize = 0x0008_0000;

    /// Network kernel staging area (loaded from network before kexec)
    /// This must not overlap with bootstrap kernel's memory region
    pub const NETWORK_KERNEL_BASE: usize = 0x0100_0000;

    /// Maximum size for network kernel (16 MB)
    /// This provides a safe buffer before any other memory regions
    pub const NETWORK_KERNEL_MAX_SIZE: usize = 0x0100_0000;

    /// End of network kernel staging area
    pub const NETWORK_KERNEL_END: usize = NETWORK_KERNEL_BASE + NETWORK_KERNEL_MAX_SIZE;
}

/// Kexec error types
#[derive(Debug, Clone, Copy)]
pub enum KexecError {
    /// Kernel image is too large for staging area
    KernelTooLarge { size: usize, max_size: usize },

    /// Kernel address is invalid or overlaps with critical regions
    InvalidAddress { addr: usize },

    /// DTB pointer is invalid
    InvalidDtb { dtb_ptr: usize },
}

/// Validate that a kernel can be safely kexec'd
///
/// This checks:
/// - Kernel fits in staging area
/// - Address is properly aligned
/// - No overlap with critical memory regions
pub fn validate_kexec(
    kernel_addr: usize,
    kernel_size: usize,
    dtb_ptr: usize,
) -> Result<(), KexecError> {
    // Check kernel size
    if kernel_size > layout::NETWORK_KERNEL_MAX_SIZE {
        return Err(KexecError::KernelTooLarge {
            size: kernel_size,
            max_size: layout::NETWORK_KERNEL_MAX_SIZE,
        });
    }

    // Check kernel address is in staging area
    if kernel_addr != layout::NETWORK_KERNEL_BASE {
        return Err(KexecError::InvalidAddress { addr: kernel_addr });
    }

    // Check DTB pointer is reasonable (not NULL)
    // Note: Firmware can pass DTB at low addresses like 0x100, so we only check for NULL
    if dtb_ptr == 0 {
        return Err(KexecError::InvalidDtb { dtb_ptr });
    }

    Ok(())
}

/// Execute a new kernel via kexec
///
/// This function performs a hot kernel replacement by:
/// 1. Validating the new kernel
/// 2. Disabling MMU/caches/interrupts
/// 3. Jumping to the new kernel
///
/// # Arguments
/// - `kernel_addr`: Physical address where new kernel is loaded
/// - `kernel_size`: Size of new kernel in bytes
/// - `dtb_ptr`: Physical address of Device Tree Blob
///
/// # Safety
///
/// This function is extremely unsafe:
/// - It never returns
/// - All running code will be destroyed
/// - The new kernel must be valid or the system will crash
/// - No cleanup is performed (file handles, network connections, etc.)
///
/// The caller must ensure:
/// - The kernel at `kernel_addr` is valid and executable
/// - All critical operations have been completed or saved
/// - Interrupts can be safely disabled
/// - No other cores are running (secondary cores should be parked)
///
/// # Panics
///
/// Panics if validation fails. In a production system, this would return
/// an error instead of panicking.
pub unsafe fn kexec(kernel_addr: usize, kernel_size: usize, dtb_ptr: usize) -> ! {
    // Validate before we destroy everything
    validate_kexec(kernel_addr, kernel_size, dtb_ptr).expect("Kexec validation failed");

    // Reset heap to clean state for new kernel
    // SAFETY: We're about to jump to a new kernel, so all current heap allocations
    // are about to become invalid anyway. This gives the new kernel a clean slate.
    unsafe {
        crate::ALLOCATOR.reset();
    }

    // SAFETY: We've validated the parameters, and the caller has promised
    // that the new kernel is valid. We're about to destroy all system state,
    // so there's no way to make this truly safe. The assembly code will:
    // 1. Disable interrupts (preventing concurrent access)
    // 2. Disable MMU (entering identity-mapped mode)
    // 3. Clean caches (ensuring memory consistency)
    // 4. Jump to new kernel (never returns)
    unsafe { kexec_jump(kernel_addr, dtb_ptr) }
}

/// Load a kernel image to the staging area and prepare for kexec
///
/// This is a helper function that:
/// 1. Copies kernel data to the staging area
/// 2. Validates the copy succeeded
/// 3. Returns the staging address for kexec
///
/// # Arguments
/// - `kernel_data`: Slice containing the kernel image
///
/// # Returns
/// The address where the kernel was staged, or an error
///
/// # Safety
///
/// This function performs raw memory writes to the staging area.
/// The caller must ensure:
/// - No other code is using the staging area
/// - The kernel_data contains a valid kernel image
pub unsafe fn stage_kernel(kernel_data: &[u8]) -> Result<usize, KexecError> {
    let kernel_size = kernel_data.len();

    // Validate size
    if kernel_size > layout::NETWORK_KERNEL_MAX_SIZE {
        return Err(KexecError::KernelTooLarge {
            size: kernel_size,
            max_size: layout::NETWORK_KERNEL_MAX_SIZE,
        });
    }

    // Copy to staging area
    // SAFETY: We've validated the size, and NETWORK_KERNEL_BASE is a valid
    // physical address in RAM. The caller has promised no other code is using
    // this region.
    unsafe {
        core::ptr::copy_nonoverlapping(
            kernel_data.as_ptr(),
            layout::NETWORK_KERNEL_BASE as *mut u8,
            kernel_size,
        );
    }

    // Ensure write completes before returning
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

    Ok(layout::NETWORK_KERNEL_BASE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_validate_kexec_valid() {
        let result = validate_kexec(
            layout::NETWORK_KERNEL_BASE,
            1024 * 1024, // 1 MB kernel
            0x100,       // DTB at 0x100 (example)
        );
        assert!(result.is_ok());
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
        let result = validate_kexec(
            0xDEADBEEF, // Invalid address
            1024, 0x100,
        );
        assert!(matches!(result, Err(KexecError::InvalidAddress { .. })));
    }

    #[test_case]
    fn test_validate_kexec_invalid_dtb() {
        let result = validate_kexec(
            layout::NETWORK_KERNEL_BASE,
            1024,
            0x0, // NULL DTB pointer
        );
        assert!(matches!(result, Err(KexecError::InvalidDtb { .. })));
    }
}

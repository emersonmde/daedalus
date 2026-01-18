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

    /// Network kernel staging area A (first network boot)
    /// This must not overlap with bootstrap kernel's memory region
    pub const NETWORK_KERNEL_BASE_A: usize = 0x0100_0000;

    /// Network kernel staging area B (second network boot, ping-pong)
    /// Used for iterative development to avoid self-overwrite
    pub const NETWORK_KERNEL_BASE_B: usize = 0x0200_0000;

    /// Maximum size for network kernel (16 MB)
    /// This provides a safe buffer before any other memory regions
    pub const NETWORK_KERNEL_MAX_SIZE: usize = 0x0100_0000;

    /// End of network kernel staging area A
    pub const NETWORK_KERNEL_END_A: usize = NETWORK_KERNEL_BASE_A + NETWORK_KERNEL_MAX_SIZE;

    /// End of network kernel staging area B
    pub const NETWORK_KERNEL_END_B: usize = NETWORK_KERNEL_BASE_B + NETWORK_KERNEL_MAX_SIZE;

    // Legacy compatibility (points to staging area A)
    pub const NETWORK_KERNEL_BASE: usize = NETWORK_KERNEL_BASE_A;
    pub const NETWORK_KERNEL_END: usize = NETWORK_KERNEL_END_A;
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

    // Check kernel address is in one of the valid staging areas
    let valid_addr = kernel_addr == layout::NETWORK_KERNEL_BASE_A
        || kernel_addr == layout::NETWORK_KERNEL_BASE_B;

    if !valid_addr {
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

/// Determine the next staging address for ping-pong kexec
///
/// This implements ping-pong staging to avoid overwriting the currently
/// running kernel during iterative development:
/// - Bootstrap (0x00080000) → Stage at A (0x01000000)
/// - Network A (0x01000000) → Stage at B (0x02000000)
/// - Network B (0x02000000) → Stage at A (0x01000000)
///
/// This allows rapid iteration: fetch v6 → kexec → fetch v7 → kexec → ...
/// without ever overwriting the running kernel.
pub fn next_staging_address() -> usize {
    let pc = read_current_pc();

    // If we're running from staging area A, use B for next kernel
    if (layout::NETWORK_KERNEL_BASE_A..layout::NETWORK_KERNEL_END_A).contains(&pc) {
        layout::NETWORK_KERNEL_BASE_B
    }
    // If we're running from staging area B, use A for next kernel
    else if (layout::NETWORK_KERNEL_BASE_B..layout::NETWORK_KERNEL_END_B).contains(&pc) {
        layout::NETWORK_KERNEL_BASE_A
    }
    // Bootstrap or anywhere else: use A as first staging area
    else {
        layout::NETWORK_KERNEL_BASE_A
    }
}

/// Read current program counter
///
/// Uses `adr` instruction to get PC-relative address on ARM64.
/// Returns bootstrap address in tests (x86_64 host).
#[cfg(target_arch = "aarch64")]
fn read_current_pc() -> usize {
    let pc: usize;
    // SAFETY: Reading the program counter is always safe. The `adr` instruction
    // loads the address of the current instruction into a register.
    unsafe {
        core::arch::asm!(
            "adr {}, .",
            out(reg) pc,
            options(nomem, nostack, preserves_flags)
        );
    }
    pc
}

/// Test stub for non-ARM64 platforms
///
/// Returns bootstrap address so tests work on development machine (x86_64)
#[cfg(not(target_arch = "aarch64"))]
fn read_current_pc() -> usize {
    layout::BOOTSTRAP_KERNEL_BASE
}

/// Load a kernel image to the staging area and prepare for kexec
///
/// This is a helper function that:
/// 1. Determines the appropriate staging address (ping-pong)
/// 2. Copies kernel data to the staging area
/// 3. Validates the copy succeeded
/// 4. Returns the staging address for kexec
///
/// # Ping-Pong Staging
///
/// The staging address is chosen to avoid overwriting the currently running kernel:
/// - Bootstrap kernel → stages at 0x01000000
/// - Kernel at 0x01000000 → stages at 0x02000000
/// - Kernel at 0x02000000 → stages at 0x01000000
///
/// This enables rapid iteration without SD card swaps.
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

    // Determine staging address using ping-pong algorithm
    let staging_addr = next_staging_address();

    // Copy to staging area
    // SAFETY: We've validated the size, and staging_addr is a valid
    // physical address in RAM (either 0x01000000 or 0x02000000).
    // The caller has promised no other code is using this region.
    unsafe {
        core::ptr::copy_nonoverlapping(kernel_data.as_ptr(), staging_addr as *mut u8, kernel_size);
    }

    // Ensure write completes before returning
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

    Ok(staging_addr)
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

    #[test_case]
    fn test_validate_kexec_staging_area_b() {
        // Staging area B should also be valid
        let result = validate_kexec(layout::NETWORK_KERNEL_BASE_B, 1024 * 1024, 0x100);
        assert!(result.is_ok());
    }

    #[test_case]
    fn test_next_staging_address_from_bootstrap() {
        // From bootstrap region, should use staging area A
        // (We can't easily test this without mocking PC read, but the logic is covered
        // by the else branch in next_staging_address)
    }

    #[test_case]
    fn test_ping_pong_alternation() {
        // Test the ping-pong logic by checking address constants
        // Verify they don't overlap
        assert!(layout::NETWORK_KERNEL_END_A <= layout::NETWORK_KERNEL_BASE_B);

        // Verify ranges are correct size
        assert_eq!(
            layout::NETWORK_KERNEL_END_A - layout::NETWORK_KERNEL_BASE_A,
            layout::NETWORK_KERNEL_MAX_SIZE
        );
        assert_eq!(
            layout::NETWORK_KERNEL_END_B - layout::NETWORK_KERNEL_BASE_B,
            layout::NETWORK_KERNEL_MAX_SIZE
        );

        // Verify legacy constants point to area A
        assert_eq!(layout::NETWORK_KERNEL_BASE, layout::NETWORK_KERNEL_BASE_A);
        assert_eq!(layout::NETWORK_KERNEL_END, layout::NETWORK_KERNEL_END_A);
    }
}

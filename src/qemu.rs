//! QEMU-specific utilities for development and testing.
//!
//! This module provides functionality that only works in QEMU, such as
//! semihosting-based exit codes for test automation.

use core::arch::asm;

extern crate alloc;
use alloc::vec::Vec;

/// Exit codes for QEMU test runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ExitCode {
    /// Test or program succeeded.
    Success = 0,
    /// Test or program failed.
    Failed = 1,
}

/// Exit QEMU using ARM semihosting with proper parameter block.
///
/// This only works when running under QEMU with semihosting enabled (-semihosting flag).
/// On real hardware, this will halt the CPU.
///
/// ARM semihosting requires a parameter block for SYS_EXIT:
/// - param\[0\] = 0x20026 (ADP_Stopped_ApplicationExit)
/// - param\[1\] = exit status code
pub fn exit(exit_code: ExitCode) -> ! {
    #[repr(C)]
    struct ExitBlock {
        reason: u64, // 0x20026 = ADP_Stopped_ApplicationExit
        status: u64, // Exit status code
    }

    let block = ExitBlock {
        reason: 0x20026, // ADP_Stopped_ApplicationExit
        status: exit_code as u64,
    };

    // SAFETY: ARM semihosting call is safe because:
    // 1. HLT #0xF000 is the standard AArch64 semihosting instruction:
    //    - Reference: https://github.com/ARM-software/abi-aa/blob/main/semihosting/semihosting.rst
    //    - Section 2.1.2: "On A64, HLT 0xF000 is used for the semihosting call"
    // 2. w0 = 0x18 (SYS_EXIT) is a valid semihosting operation:
    //    - Reference: https://developer.arm.com/documentation/dui0003/latest/semihosting-operations/sys_exit-0x18
    //    - Reports exception/exit to debugger
    // 3. x1 points to a valid ExitBlock struct on the stack with correct layout
    // 4. ExitBlock layout matches ARM semihosting requirements (#[repr(C)])
    //    - param[0] = 0x20026 (ADP_Stopped_ApplicationExit)
    //    - param[1] = exit status code
    // 5. options(noreturn) correctly indicates this never returns
    // 6. In QEMU with semihosting: exits cleanly; On real hardware: halts CPU (safe)
    unsafe {
        asm!(
            "mov w0, #0x18",           // SYS_EXIT
            "mov x1, {0}",             // x1 = address of parameter block
            "hlt #0xf000",             // Semihosting call
            in(reg) &block,
            options(noreturn)
        );
    }
}

/// Semihosting file operations for QEMU
pub mod semihosting {
    use super::*;
    use alloc::vec;

    #[derive(Debug)]
    pub enum SemihostingError {
        OpenFailed,
        ReadFailed,
        FileTooLarge,
    }

    /// Read entire file using QEMU semihosting
    ///
    /// This only works in QEMU with -semihosting flag.
    /// On real hardware, this will fail.
    pub fn read_file(filename: &str) -> Result<Vec<u8>, SemihostingError> {
        // SYS_OPEN (0x01) - Open file
        // Parameters: [filename_ptr, filename_len, mode]
        // mode: 0 = read, 4 = binary read
        let fd = unsafe {
            let params = [filename.as_ptr() as u64, filename.len() as u64, 0u64];

            let result: i64;
            asm!(
                "mov w0, #0x01",           // SYS_OPEN
                "mov x1, {0}",             // x1 = params
                "hlt #0xf000",             // Semihosting call
                "mov {1}, x0",             // Get return value
                in(reg) params.as_ptr(),
                out(reg) result,
            );

            if result < 0 {
                return Err(SemihostingError::OpenFailed);
            }
            result as u64
        };

        // SYS_FLEN (0x0C) - Get file length
        let file_len = unsafe {
            let result: i64;
            asm!(
                "mov w0, #0x0C",           // SYS_FLEN
                "mov x1, {0}",             // x1 = file descriptor
                "hlt #0xf000",
                "mov {1}, x0",
                in(reg) fd,
                out(reg) result,
            );

            if result < 0 {
                return Err(SemihostingError::ReadFailed);
            }
            result as usize
        };

        // Limit file size to 10 MB for safety
        if file_len > 10 * 1024 * 1024 {
            return Err(SemihostingError::FileTooLarge);
        }

        // Allocate initialized buffer (clippy::uninit_vec)
        let mut buffer = vec![0u8; file_len];

        // SYS_READ (0x06) - Read from file
        let bytes_read = unsafe {
            let params = [fd, buffer.as_mut_ptr() as u64, buffer.len() as u64];

            let result: i64;
            asm!(
                "mov w0, #0x06",           // SYS_READ
                "mov x1, {0}",             // x1 = params
                "hlt #0xf000",
                "mov {1}, x0",
                in(reg) params.as_ptr(),
                out(reg) result,
            );

            result
        };

        // SYS_CLOSE (0x02) - Close file
        unsafe {
            asm!(
                "mov w0, #0x02",           // SYS_CLOSE
                "mov x1, {0}",             // x1 = file descriptor
                "hlt #0xf000",
                in(reg) fd,
            );
        }

        // Check if we read the expected amount
        if bytes_read < 0 || bytes_read as usize != file_len {
            return Err(SemihostingError::ReadFailed);
        }

        Ok(buffer)
    }
}

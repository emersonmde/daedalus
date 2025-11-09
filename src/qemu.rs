use core::arch::asm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ExitCode {
    Success = 0,
    Failed = 1,
}

/// Exit QEMU using ARM semihosting with proper parameter block
///
/// This only works when running under QEMU with semihosting enabled (-semihosting flag).
/// On real hardware, this will halt the CPU.
///
/// ARM semihosting requires a parameter block for SYS_EXIT:
/// - param[0] = 0x20026 (ADP_Stopped_ApplicationExit)
/// - param[1] = exit status code
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

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

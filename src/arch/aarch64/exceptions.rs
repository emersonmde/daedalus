//! AArch64 exception handling.
//!
//! Provides exception vector installation and handlers for synchronous exceptions,
//! IRQs, FIQs, and SErrors. Handlers print detailed exception context including
//! register dumps and exception syndrome information.

use crate::println;
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

/// ARM instruction size in bytes (all ARMv8-A instructions are 32-bit/4-byte)
const ARM_INSTRUCTION_SIZE: u64 = 4;

/// Global flag indicating we're probing hardware (set during hardware detection)
/// When true, Data Abort exceptions return 0 instead of panicking
static PROBING_HARDWARE: AtomicBool = AtomicBool::new(false);

/// ESR (Exception Syndrome Register) field definitions.
///
/// Reference: [ARM ARM ESR_EL1](https://developer.arm.com/documentation/ddi0595/2020-12/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-)
mod esr_fields {
    /// Exception Class (EC) field - bits \[31:26\]
    pub const EC_SHIFT: u32 = 26;
    pub const EC_MASK: u64 = 0x3F;

    /// Instruction Specific Syndrome (ISS) field - bits \[24:0\]
    pub const ISS_MASK: u64 = 0x1FFFFFF;
}

/// Read current exception level (EL0-EL3)
///
/// Returns the current exception level as a u8 (0, 1, 2, or 3).
/// This is a helper to avoid repeating the same unsafe block throughout the module.
fn current_el() -> u8 {
    let current_el: u64;
    // SAFETY: Reading CurrentEL system register is safe because:
    // 1. CurrentEL is a read-only system register (ARM ARM: https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CurrentEL--Current-Exception-Level)
    // 2. MRS instruction with nomem,nostack has no side effects (only reads register value)
    // 3. CurrentEL is accessible from EL1/EL2/EL3 (we run at EL1 or EL2, never EL0/usermode)
    // 4. Bits [3:2] contain EL value, bits [63:4] and [1:0] are RES0 (reserved zero)
    unsafe {
        asm!("mrs {}, CurrentEL", out(reg) current_el, options(nomem, nostack));
    }
    ((current_el >> 2) & 0x3) as u8
}

/// Exception context saved by assembly stub
/// Layout must match SAVE_CONTEXT macro in exceptions.s
#[repr(C)]
pub struct ExceptionContext {
    pub elr_el1: u64,  // Exception Link Register (return address)
    pub x30: u64,      // Link Register
    pub spsr_el1: u64, // Saved Program Status Register
    pub x28: u64,
    pub x29: u64,
    pub x26: u64,
    pub x27: u64,
    pub x24: u64,
    pub x25: u64,
    pub x22: u64,
    pub x23: u64,
    pub x20: u64,
    pub x21: u64,
    pub x18: u64,
    pub x19: u64,
    pub x16: u64,
    pub x17: u64,
    pub x14: u64,
    pub x15: u64,
    pub x12: u64,
    pub x13: u64,
    pub x10: u64,
    pub x11: u64,
    pub x8: u64,
    pub x9: u64,
    pub x6: u64,
    pub x7: u64,
    pub x4: u64,
    pub x5: u64,
    pub x2: u64,
    pub x3: u64,
    pub x0: u64,
    pub x1: u64,
}

/// Exception types
#[derive(Debug)]
#[repr(u64)]
pub enum ExceptionType {
    Synchronous = 0,
    Irq = 1,
    Fiq = 2,
    SError = 3,
}

impl ExceptionType {
    fn from_u64(value: u64) -> Self {
        match value {
            0 => ExceptionType::Synchronous,
            1 => ExceptionType::Irq,
            2 => ExceptionType::Fiq,
            3 => ExceptionType::SError,
            _ => ExceptionType::Synchronous,
        }
    }
}

/// Exception Syndrome Register (ESR_ELx) fields
#[derive(Debug)]
pub struct ExceptionSyndrome {
    pub ec: u32,  // Exception Class
    pub iss: u32, // Instruction Specific Syndrome
}

impl ExceptionSyndrome {
    /// Read ESR for current EL
    pub fn read() -> Self {
        let el = current_el();

        let esr: u64;
        // SAFETY: Reading ESR_ELx is safe because:
        // 1. We determined the current EL above and select the appropriate ESR register for that EL
        // 2. ESR_EL1/ESR_EL2 are read-only status registers holding exception syndrome information
        //    - ESR_EL1: https://developer.arm.com/documentation/ddi0595/2020-12/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-
        //    - ESR_EL2: https://developer.arm.com/documentation/ddi0601/2022-03/AArch64-Registers/ESR-EL2--Exception-Syndrome-Register--EL2-
        // 3. Hardware populates ESR_ELx when taking exception to that EL; no side effects on read
        // 4. MRS instruction with nomem,nostack has no side effects (only reads register value)
        // 5. Reading ESR from current EL is architecturally permitted
        unsafe {
            if el == 2 {
                asm!("mrs {}, esr_el2", out(reg) esr, options(nomem, nostack));
            } else {
                asm!("mrs {}, esr_el1", out(reg) esr, options(nomem, nostack));
            }
        }
        Self {
            ec: ((esr >> esr_fields::EC_SHIFT) & esr_fields::EC_MASK) as u32,
            iss: (esr & esr_fields::ISS_MASK) as u32,
        }
    }

    /// Get exception class description
    pub fn exception_class_str(&self) -> &'static str {
        match self.ec {
            0x00 => "Unknown reason",
            0x01 => "Trapped WFI/WFE",
            0x03 => "Trapped MCR/MRC (CP15)",
            0x04 => "Trapped MCRR/MRRC (CP15)",
            0x05 => "Trapped MCR/MRC (CP14)",
            0x06 => "Trapped LDC/STC",
            0x07 => "Trapped FP/SIMD",
            0x0C => "Trapped MRRC (CP14)",
            0x0E => "Illegal Execution State",
            0x11 => "SVC instruction (AArch32)",
            0x12 => "HVC instruction (AArch32)",
            0x13 => "SMC instruction (AArch32)",
            0x15 => "SVC instruction (AArch64)",
            0x16 => "HVC instruction (AArch64)",
            0x17 => "SMC instruction (AArch64)",
            0x18 => "Trapped MSR/MRS/System instruction",
            0x1F => "Implementation defined (EL3)",
            0x20 => "Instruction Abort (lower EL)",
            0x21 => "Instruction Abort (same EL)",
            0x22 => "PC alignment fault",
            0x24 => "Data Abort (lower EL)",
            0x25 => "Data Abort (same EL)",
            0x26 => "SP alignment fault",
            0x28 => "Trapped FP (AArch32)",
            0x2C => "Trapped FP (AArch64)",
            0x2F => "SError",
            0x30 => "Breakpoint (lower EL)",
            0x31 => "Breakpoint (same EL)",
            0x32 => "Software Step (lower EL)",
            0x33 => "Software Step (same EL)",
            0x34 => "Watchpoint (lower EL)",
            0x35 => "Watchpoint (same EL)",
            0x38 => "BKPT instruction (AArch32)",
            0x3A => "Vector Catch (AArch32)",
            0x3C => "BRK instruction (AArch64)",
            _ => "Reserved/Unknown",
        }
    }
}

/// Read FAR (Faulting Address Register) for current EL
fn read_far() -> u64 {
    let el = current_el();

    let far: u64;
    // SAFETY: Reading FAR_ELx is safe because:
    // 1. We determined the current EL above and select the appropriate FAR register for that EL
    // 2. FAR_EL1/FAR_EL2 are read-only status registers holding faulting virtual address
    //    - FAR_EL1: https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/FAR-EL1--Fault-Address-Register--EL1-
    //    - FAR_EL2: https://developer.arm.com/documentation/ddi0595/latest/AArch64-Registers/FAR-EL2--Fault-Address-Register--EL2-
    // 3. Hardware populates FAR_ELx on instruction/data aborts and alignment faults; no side effects on read
    // 4. MRS instruction with nomem,nostack has no side effects (only reads register value)
    // 5. Reading FAR from current EL is architecturally permitted
    unsafe {
        if el == 2 {
            asm!("mrs {}, far_el2", out(reg) far, options(nomem, nostack));
        } else {
            asm!("mrs {}, far_el1", out(reg) far, options(nomem, nostack));
        }
    }
    far
}

/// Print exception context with register dump
fn print_exception_context(ctx: &ExceptionContext, exc_type: ExceptionType, source: &str) {
    let esr = ExceptionSyndrome::read();
    let far = read_far();

    println!("\n!!! EXCEPTION !!!");
    println!("Source: {}", source);
    println!("Type: {:?}", exc_type);
    println!(
        "Exception Class: 0x{:02x} ({})",
        esr.ec,
        esr.exception_class_str()
    );
    println!("ISS: 0x{:07x}", esr.iss);
    println!("ELR (return addr): 0x{:016x}", ctx.elr_el1);
    println!("FAR (fault addr):  0x{:016x}", far);
    println!("SPSR: 0x{:016x}", ctx.spsr_el1);
    println!("\nRegisters:");
    println!("  x0: 0x{:016x}  x1: 0x{:016x}", ctx.x0, ctx.x1);
    println!("  x2: 0x{:016x}  x3: 0x{:016x}", ctx.x2, ctx.x3);
    println!("  x4: 0x{:016x}  x5: 0x{:016x}", ctx.x4, ctx.x5);
    println!("  x6: 0x{:016x}  x7: 0x{:016x}", ctx.x6, ctx.x7);
    println!("  x8: 0x{:016x}  x9: 0x{:016x}", ctx.x8, ctx.x9);
    println!(" x10: 0x{:016x} x11: 0x{:016x}", ctx.x10, ctx.x11);
    println!(" x12: 0x{:016x} x13: 0x{:016x}", ctx.x12, ctx.x13);
    println!(" x14: 0x{:016x} x15: 0x{:016x}", ctx.x14, ctx.x15);
    println!(" x16: 0x{:016x} x17: 0x{:016x}", ctx.x16, ctx.x17);
    println!(" x18: 0x{:016x} x19: 0x{:016x}", ctx.x18, ctx.x19);
    println!(" x20: 0x{:016x} x21: 0x{:016x}", ctx.x20, ctx.x21);
    println!(" x22: 0x{:016x} x23: 0x{:016x}", ctx.x22, ctx.x23);
    println!(" x24: 0x{:016x} x25: 0x{:016x}", ctx.x24, ctx.x25);
    println!(" x26: 0x{:016x} x27: 0x{:016x}", ctx.x26, ctx.x27);
    println!(" x28: 0x{:016x} x29: 0x{:016x}", ctx.x28, ctx.x29);
    println!(" x30: 0x{:016x}", ctx.x30);
}

// ============================================================================
// Exception Handlers
// ============================================================================
// These are called from the exception vector table in exceptions.s

/// Handle exceptions from current EL using SP0.
// SAFETY: no_mangle required because this function is called by name from assembly (exceptions.s).
// extern "C" ensures stable ABI. Assembly guarantees: valid ExceptionContext pointer, valid exc_type value.
#[unsafe(no_mangle)]
extern "C" fn exception_handler_el1_sp0(ctx: &ExceptionContext, exc_type: u64) {
    print_exception_context(ctx, ExceptionType::from_u64(exc_type), "Current EL (SP0)");
    panic!("Unhandled exception");
}

/// Handle exceptions from current EL using SPx.
// SAFETY: no_mangle required because this function is called by name from assembly (exceptions.s).
// extern "C" ensures stable ABI. Assembly guarantees: valid ExceptionContext pointer, valid exc_type value.
// Context is mutable because assembly saves it on stack and restores it after handler returns.
#[unsafe(no_mangle)]
extern "C" fn exception_handler_el1_spx(ctx: &mut ExceptionContext, exc_type: u64) {
    let exc_type = ExceptionType::from_u64(exc_type);

    // Handle IRQs separately
    if matches!(exc_type, ExceptionType::Irq) {
        handle_irq();
        return;
    }

    // Log FIQ/SError for debugging (these should never happen normally)
    if matches!(exc_type, ExceptionType::Fiq) {
        crate::println!("[EXCEPTION] FIQ received!");
    } else if matches!(exc_type, ExceptionType::SError) {
        crate::println!("[EXCEPTION] SError received!");
    }

    // Handle Data Abort during hardware probing (Linux-style probe)
    // When probing hardware, return 0 instead of panicking on Data Abort
    if matches!(exc_type, ExceptionType::Synchronous) && PROBING_HARDWARE.load(Ordering::Acquire) {
        // Read ESR to check if this is a Data Abort
        let esr: u64;
        // SAFETY: Reading ESR_EL1 system register is safe (read-only, no side effects)
        unsafe {
            asm!("mrs {}, ESR_EL1", out(reg) esr, options(nomem, nostack));
        }

        let ec = (esr >> esr_fields::EC_SHIFT) & esr_fields::EC_MASK;
        // 0x25 = Data Abort from same EL
        if ec == 0x25 {
            // Clear probe flag
            PROBING_HARDWARE.store(false, Ordering::Release);

            // Modify exception context to skip faulting instruction and return 0
            // We have &mut access to the context, which will be restored by assembly (RESTORE_CONTEXT).
            // This allows MMIO probe to recover from faults gracefully:
            // - ELR += ARM_INSTRUCTION_SIZE skips the faulting LDR instruction
            // - x0 = 0 provides a safe return value indicating hardware not present
            ctx.elr_el1 += ARM_INSTRUCTION_SIZE;
            ctx.x0 = 0; // Return 0 for failed read
            return; // Return without panicking
        }
    }

    // All other exceptions: print context and panic
    print_exception_context(ctx, exc_type, "Current EL (SPx)");
    panic!("Unhandled exception");
}

/// Handle exceptions from lower EL in AArch64 mode.
// SAFETY: no_mangle required because this function is called by name from assembly (exceptions.s).
// extern "C" ensures stable ABI. Assembly guarantees: valid ExceptionContext pointer, valid exc_type value.
#[unsafe(no_mangle)]
extern "C" fn exception_handler_lower_aa64(ctx: &ExceptionContext, exc_type: u64) {
    print_exception_context(ctx, ExceptionType::from_u64(exc_type), "Lower EL (AArch64)");
    panic!("Unhandled exception");
}

/// Handle exceptions from lower EL in AArch32 mode.
// SAFETY: no_mangle required because this function is called by name from assembly (exceptions.s).
// extern "C" ensures stable ABI. Assembly guarantees: valid ExceptionContext pointer, valid exc_type value.
#[unsafe(no_mangle)]
extern "C" fn exception_handler_lower_aa32(ctx: &ExceptionContext, exc_type: u64) {
    print_exception_context(ctx, ExceptionType::from_u64(exc_type), "Lower EL (AArch32)");
    panic!("Unhandled exception");
}

//-----------------------------------------------------------------------------
// IRQ handling
//-----------------------------------------------------------------------------

/// Handle an IRQ by reading the interrupt ID from the GIC and routing to
/// the appropriate peripheral handler.
fn handle_irq() {
    // Acknowledge the interrupt and get its ID
    // Drop the lock immediately after acknowledging
    let int_id = {
        let gic = crate::drivers::gic::GIC.lock();
        gic.acknowledge_interrupt()
    }; // GIC lock dropped here

    // Spurious interrupt check (ID 1023 means no pending interrupt)
    if int_id == 1023 {
        static SPURIOUS_COUNT: core::sync::atomic::AtomicU32 =
            core::sync::atomic::AtomicU32::new(0);
        let count = SPURIOUS_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        if count < 5 {
            crate::println!("[IRQ] Spurious interrupt (ID 1023, count: {})", count + 1);
        }
        return;
    }

    // Route to appropriate handler based on interrupt ID
    // This runs without holding the GIC lock, allowing other code to query GIC if needed
    match int_id {
        crate::drivers::gic::irq::UART0 => {
            crate::drivers::uart::handle_interrupt();
        }
        crate::drivers::gic::irq::GENET_0 | crate::drivers::gic::irq::GENET_1 => {
            crate::drivers::genet::handle_interrupt();
        }
        _ => {
            // Unknown interrupt - log it for debugging (limit to first few to avoid spam)
            static UNKNOWN_IRQ_COUNT: core::sync::atomic::AtomicU32 =
                core::sync::atomic::AtomicU32::new(0);
            let count = UNKNOWN_IRQ_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            if count < 10 {
                crate::println!(
                    "[IRQ] Unknown interrupt ID: {} (count: {})",
                    int_id,
                    count + 1
                );
            }
        }
    }

    // Signal end of interrupt to GIC
    // Re-acquire the lock just for the EOI write
    let gic = crate::drivers::gic::GIC.lock();
    gic.end_of_interrupt(int_id);
}

//-----------------------------------------------------------------------------
// Exception vector table installation
//-----------------------------------------------------------------------------

// SAFETY: Linker symbol from exceptions.s marking the base address of the exception vector table.
// 1. Symbol is defined in src/arch/aarch64/exceptions.s with .align 11 (2KB alignment requirement)
// 2. Vector table is placed in executable section by linker (valid code address)
// 3. This is not a real variable, just an address marker - never dereferenced directly
// 4. Used only to get its address via &exception_vector_table as *const u64 as u64
// 5. ARM requires VBAR to point to 2KB-aligned exception vectors (enforced by assembler)
unsafe extern "C" {
    static exception_vector_table: u64;
}

/// Install the exception vector table by setting VBAR_EL1 or VBAR_EL2
pub fn init() {
    // Check current exception level
    let el = current_el();

    // SAFETY: Setting VBAR_ELx is safe because:
    // 1. We determined the current EL above and select the appropriate VBAR register for that EL
    // 2. VBAR_EL1/VBAR_EL2 are read-write registers holding exception vector table base address
    //    - VBAR_EL1: <https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/VBAR-EL1--Vector-Base-Address-Register--EL1->
    //    - VBAR_EL2: <https://developer.arm.com/documentation/ddi0595/2020-12/AArch64-Registers/VBAR-EL2--Vector-Base-Address-Register--EL2->
    // 3. exception_vector_table is a valid symbol defined in exceptions.s by the assembler
    // 4. The vector table is properly aligned (2KB alignment enforced by .align 11 directive in assembly)
    // 5. We're setting VBAR for the current EL, which is architecturally permitted
    // 6. ISB (Instruction Synchronization Barrier) ensures all subsequent instructions see the new VBAR value
    // 7. MSR with nomem,nostack only writes to system register (no memory/stack access)
    unsafe {
        let vbar = &exception_vector_table as *const u64 as u64;

        // Set VBAR for the current exception level
        if el == 2 {
            asm!(
                "msr vbar_el2, {}",
                "isb",
                in(reg) vbar,
                options(nomem, nostack)
            );
        } else {
            asm!(
                "msr vbar_el1, {}",
                "isb",
                in(reg) vbar,
                options(nomem, nostack)
            );
        }
    }
}

/// Safely probe a hardware register address (Linux-style probe)
///
/// Attempts to read from a memory-mapped I/O address. If the address triggers a
/// Data Abort (hardware not present), returns 0 instead of panicking.
///
/// This is similar to Linux's `probe_kernel_read` - it sets a flag before reading,
/// and the exception handler checks this flag to recover gracefully from Data Aborts.
///
/// # Arguments
/// * `addr` - Physical address to probe (e.g., hardware register)
///
/// # Returns
/// * `u32` - Register value if hardware present, 0 if Data Abort occurred
///
/// # Safety
/// This function uses exception handling to recover from invalid memory access.
///
/// # Requirements
/// - Address must be 4-byte aligned (ARMv8-A requires aligned access for LDR instruction)
/// - Misaligned addresses will cause Alignment Fault instead of Data Abort
///
/// # Example
/// ```ignore
/// let version = probe_read_u32(0xFD580000); // GENET version register (aligned)
/// if version != 0 {
///     // Hardware is present
/// }
/// ```
pub fn probe_read_u32(addr: usize) -> u32 {
    // Set probe flag before attempting read
    PROBING_HARDWARE.store(true, Ordering::Release);

    // Attempt to read - if Data Abort occurs, exception handler will:
    // 1. Clear PROBING_HARDWARE flag
    // 2. Set return value (x0) to 0
    // 3. Skip past the faulting instruction (ELR += ARM_INSTRUCTION_SIZE)
    // 4. Return normally
    //
    // SAFETY: We set PROBING_HARDWARE flag so exception handler knows to recover from Data Abort.
    // If hardware not present, exception handler modifies return value to 0.
    // Address must be 4-byte aligned (caller's responsibility).
    // read_volatile is required to prevent compiler optimization.
    let value = unsafe { core::ptr::read_volatile(addr as *const u32) };

    // Clear probe flag on successful read
    PROBING_HARDWARE.store(false, Ordering::Release);

    value
}

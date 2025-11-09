// AArch64 Exception Vector Table
// ARMv8-A Architecture Reference Manual D1.10.2
//
// Exception vector table must be aligned to 2048 bytes (0x800)
// Each entry is 128 bytes (0x80) in size
// 16 entries total: 4 exception types × 4 exception levels

.section .text.exceptions

// Align to 2048 bytes (2^11)
.align 11

.global exception_vector_table
exception_vector_table:

// Current EL with SP0
// Entry 0x000: Synchronous
.align 7
    b exc_sync_el1_sp0
// Entry 0x080: IRQ
.align 7
    b exc_irq_el1_sp0
// Entry 0x100: FIQ
.align 7
    b exc_fiq_el1_sp0
// Entry 0x180: SError
.align 7
    b exc_serror_el1_sp0

// Current EL with SPx (our normal case - EL1 with SP_EL1)
// Entry 0x200: Synchronous
.align 7
    b exc_sync_el1_spx
// Entry 0x280: IRQ
.align 7
    b exc_irq_el1_spx
// Entry 0x300: FIQ
.align 7
    b exc_fiq_el1_spx
// Entry 0x380: SError
.align 7
    b exc_serror_el1_spx

// Lower EL using AArch64
// Entry 0x400: Synchronous
.align 7
    b exc_sync_lower_aa64
// Entry 0x480: IRQ
.align 7
    b exc_irq_lower_aa64
// Entry 0x500: FIQ
.align 7
    b exc_fiq_lower_aa64
// Entry 0x580: SError
.align 7
    b exc_serror_lower_aa64

// Lower EL using AArch32
// Entry 0x600: Synchronous
.align 7
    b exc_sync_lower_aa32
// Entry 0x680: IRQ
.align 7
    b exc_irq_lower_aa32
// Entry 0x700: FIQ
.align 7
    b exc_fiq_lower_aa32
// Entry 0x780: SError
.align 7
    b exc_serror_lower_aa32

//-----------------------------------------------------------------------------
// Exception Handler Stubs
//-----------------------------------------------------------------------------
// Each handler saves context and calls into Rust

// Macro to save all general-purpose registers
.macro SAVE_CONTEXT
    // Save all general-purpose registers to stack
    stp x0,  x1,  [sp, #-16]!
    stp x2,  x3,  [sp, #-16]!
    stp x4,  x5,  [sp, #-16]!
    stp x6,  x7,  [sp, #-16]!
    stp x8,  x9,  [sp, #-16]!
    stp x10, x11, [sp, #-16]!
    stp x12, x13, [sp, #-16]!
    stp x14, x15, [sp, #-16]!
    stp x16, x17, [sp, #-16]!
    stp x18, x19, [sp, #-16]!
    stp x20, x21, [sp, #-16]!
    stp x22, x23, [sp, #-16]!
    stp x24, x25, [sp, #-16]!
    stp x26, x27, [sp, #-16]!
    stp x28, x29, [sp, #-16]!

    // Save link register (x30) and saved program status register
    mrs x0, spsr_el1
    stp x30, x0, [sp, #-16]!

    // Save exception link register (return address)
    mrs x0, elr_el1
    str x0, [sp, #-8]!
.endm

// Macro to restore all general-purpose registers
.macro RESTORE_CONTEXT
    // Restore exception link register
    ldr x0, [sp], #8
    msr elr_el1, x0

    // Restore link register and spsr
    ldp x30, x0, [sp], #16
    msr spsr_el1, x0

    // Restore general-purpose registers
    ldp x28, x29, [sp], #16
    ldp x26, x27, [sp], #16
    ldp x24, x25, [sp], #16
    ldp x22, x23, [sp], #16
    ldp x20, x21, [sp], #16
    ldp x18, x19, [sp], #16
    ldp x16, x17, [sp], #16
    ldp x14, x15, [sp], #16
    ldp x12, x13, [sp], #16
    ldp x10, x11, [sp], #16
    ldp x8,  x9,  [sp], #16
    ldp x6,  x7,  [sp], #16
    ldp x4,  x5,  [sp], #16
    ldp x2,  x3,  [sp], #16
    ldp x0,  x1,  [sp], #16
.endm

//-----------------------------------------------------------------------------
// Current EL with SP0 handlers
//-----------------------------------------------------------------------------

exc_sync_el1_sp0:
    SAVE_CONTEXT
    mov x0, sp                      // Pass context pointer to handler
    mov x1, #0                      // Exception type: Sync
    bl exception_handler_el1_sp0
    RESTORE_CONTEXT
    eret

exc_irq_el1_sp0:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #1                      // Exception type: IRQ
    bl exception_handler_el1_sp0
    RESTORE_CONTEXT
    eret

exc_fiq_el1_sp0:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #2                      // Exception type: FIQ
    bl exception_handler_el1_sp0
    RESTORE_CONTEXT
    eret

exc_serror_el1_sp0:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #3                      // Exception type: SError
    bl exception_handler_el1_sp0
    RESTORE_CONTEXT
    eret

//-----------------------------------------------------------------------------
// Current EL with SPx handlers (our normal case)
//-----------------------------------------------------------------------------

exc_sync_el1_spx:
    SAVE_CONTEXT
    mov x0, sp                      // Pass context pointer to handler
    mov x1, #0                      // Exception type: Sync
    bl exception_handler_el1_spx
    RESTORE_CONTEXT
    eret

exc_irq_el1_spx:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #1                      // Exception type: IRQ
    bl exception_handler_el1_spx
    RESTORE_CONTEXT
    eret

exc_fiq_el1_spx:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #2                      // Exception type: FIQ
    bl exception_handler_el1_spx
    RESTORE_CONTEXT
    eret

exc_serror_el1_spx:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #3                      // Exception type: SError
    bl exception_handler_el1_spx
    RESTORE_CONTEXT
    eret

//-----------------------------------------------------------------------------
// Lower EL (AArch64) handlers
//-----------------------------------------------------------------------------

exc_sync_lower_aa64:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #0
    bl exception_handler_lower_aa64
    RESTORE_CONTEXT
    eret

exc_irq_lower_aa64:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #1
    bl exception_handler_lower_aa64
    RESTORE_CONTEXT
    eret

exc_fiq_lower_aa64:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #2
    bl exception_handler_lower_aa64
    RESTORE_CONTEXT
    eret

exc_serror_lower_aa64:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #3
    bl exception_handler_lower_aa64
    RESTORE_CONTEXT
    eret

//-----------------------------------------------------------------------------
// Lower EL (AArch32) handlers
//-----------------------------------------------------------------------------

exc_sync_lower_aa32:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #0
    bl exception_handler_lower_aa32
    RESTORE_CONTEXT
    eret

exc_irq_lower_aa32:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #1
    bl exception_handler_lower_aa32
    RESTORE_CONTEXT
    eret

exc_fiq_lower_aa32:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #2
    bl exception_handler_lower_aa32
    RESTORE_CONTEXT
    eret

exc_serror_lower_aa32:
    SAVE_CONTEXT
    mov x0, sp
    mov x1, #3
    bl exception_handler_lower_aa32
    RESTORE_CONTEXT
    eret

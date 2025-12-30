// AArch64 boot stub for Raspberry Pi 4
// Entry point called by Pi firmware at 0x80000

.section .text.boot

.global _start

_start:
    // Read MPIDR_EL1 to get core ID
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF       // Extract Aff0 field (core ID)
    cbz     x0, primary_core    // If core 0, continue

    // Secondary cores: park in infinite loop
park:
    wfe                         // Wait for event
    b       park

primary_core:
    // Drop from EL2 to EL1 if currently at EL2
    mrs     x0, CurrentEL
    and     x0, x0, #0xC        // Bits [3:2] contain EL
    cmp     x0, #8              // EL2 = 0b10 << 2 = 8
    b.ne    setup_stack         // If not EL2, skip transition

    // Initialize EL1 system registers (have UNKNOWN values before first entry to EL1)
    // Reference: ARM-TF lib/el3_runtime/aarch64/context_mgmt.c

    // SCTLR_EL1: Set RES1 bits, MMU/caches disabled
    // Value from ARM Trusted Firmware: SCTLR_EL1_RES1 = 0x30D00800
    ldr     x0, =0x30D00800
    msr     sctlr_el1, x0

    // Initialize MMU-related registers to zero (safe disabled state)
    msr     tcr_el1, xzr        // Translation Control Register
    msr     mair_el1, xzr       // Memory Attribute Indirection Register
    msr     ttbr0_el1, xzr      // Translation Table Base Register 0
    msr     ttbr1_el1, xzr      // Translation Table Base Register 1

    // Enable FP/SIMD at EL1 (CPACR_EL1.FPEN = 0b11)
    // LLVM may use SIMD for memory operations in Rust code
    mov     x0, #(0b11 << 20)   // FPEN: No trapping of FP/SIMD instructions
    msr     cpacr_el1, x0

    // Initialize VBAR_EL1 to point to exception vector table
    // This prevents crashes if any exception occurs before Rust installs handlers
    // Use ldr = (not adr) because exception_vector_table is in exceptions.s (different file)
    ldr     x0, =exception_vector_table
    msr     vbar_el1, x0

    isb                         // Synchronize context

    // HCR_EL2: Configure EL1 execution state
    mov     x0, #(1 << 31)      // RW bit: EL1 is AArch64
    msr     hcr_el2, x0

    // SPSR_EL2: Set exception level and mask interrupts
    mov     x0, #0x3C5          // EL1h mode, all interrupts masked
    msr     spsr_el2, x0

    // ELR_EL2: Set return address
    adr     x0, setup_stack
    msr     elr_el2, x0

    // Return to EL1
    eret

setup_stack:
    // Set up stack pointer
    ldr     x0, =_stack_start
    mov     sp, x0

    // Clear BSS section
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end

clear_bss:
    cmp     x0, x1
    b.ge    clear_bss_done
    str     xzr, [x0], #8       // Store zero and increment
    b       clear_bss

clear_bss_done:
    // Jump to Rust entry point
    bl      _start_rust

    // If Rust returns (it shouldn't), park
halt:
    wfe
    b       halt

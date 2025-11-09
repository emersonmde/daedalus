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

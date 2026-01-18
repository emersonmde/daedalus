// AArch64 kexec implementation - hot kernel replacement
// This stub disables MMU/caches/interrupts and jumps to a new kernel
//
// Function signature:
//   void kexec_jump(usize new_kernel_addr, usize dtb_ptr)
//
// Reference: ARM DDI 0487 (ARMv8-A Architecture Reference Manual)
// - D13.2: System Control Register (SCTLR_EL1)
// - D8.10: TLB maintenance operations
// - D4.4: Cache maintenance operations

.section .text

.global kexec_jump

kexec_jump:
    // Arguments:
    //   x0 = new_kernel_addr (address to jump to)
    //   x1 = dtb_ptr (Device Tree Blob pointer to pass to new kernel)

    // Save new kernel address and DTB pointer
    mov     x20, x0             // x20 = new kernel address (callee-saved)
    mov     x19, x1             // x19 = DTB pointer (callee-saved)

    // Step 1: Disable all interrupts
    // MSR DAIFSET, #15 - Set all DAIF bits (Debug, SError, IRQ, FIQ)
    // Reference: ARM DDI 0487, C5.2.7 (Interrupt Mask Bits)
    msr     daifset, #15

    // Step 2: Disable MMU and data cache
    // SCTLR_EL1 bit 0 = M (MMU enable)
    // SCTLR_EL1 bit 2 = C (Data cache enable)
    // SCTLR_EL1 bit 12 = I (Instruction cache enable)
    // Reference: ARM DDI 0487, D13.2.118
    mrs     x2, sctlr_el1
    bic     x2, x2, #(1 << 0)   // Clear M bit (disable MMU)
    bic     x2, x2, #(1 << 2)   // Clear C bit (disable data cache)
    bic     x2, x2, #(1 << 12)  // Clear I bit (disable instruction cache)
    msr     sctlr_el1, x2
    isb                         // Synchronize context

    // Step 3: Invalidate entire TLB
    // TLBI VMALLE1 - Invalidate all stage 1 translations for EL1
    // Reference: ARM DDI 0487, D8.10.1
    tlbi    vmalle1
    dsb     sy                  // Ensure TLB invalidation completes
    isb

    // Step 4: Clean and invalidate entire data cache
    // We need to clean D-cache to ensure all dirty data is written to memory
    // Then invalidate so new kernel doesn't see stale cache lines
    //
    // This is a simplified version - production code would walk cache hierarchy
    // For now, use point-of-coherency maintenance
    // Reference: ARM DDI 0487, D4.4

    // Clean entire data cache to PoC (Point of Coherency)
    // Cache maintenance by set/way for all cache levels
    // Note: This is a simplified implementation assuming L1/L2 caches

    // Get cache level info
    mrs     x2, clidr_el1       // Cache Level ID Register
    and     x3, x2, #0x7        // Extract LoC (Level of Coherency)
    cbz     x3, cache_done      // If LoC == 0, no caches to clean

    // For simplicity, just do DC CIVAC for the kernel region
    // In production, would walk all cache sets/ways
    // CIVAC = Clean and Invalidate by VA to PoC
    mov     x2, #0x00080000     // Bootstrap kernel start
    mov     x3, #0x04280000     // Bootstrap kernel end (approx)

clean_loop:
    dc      civac, x2           // Clean and invalidate cache line
    add     x2, x2, #64         // Cache line size = 64 bytes
    cmp     x2, x3
    b.lt    clean_loop

cache_done:
    // Step 5: Invalidate entire instruction cache
    // IC IALLUIS - Invalidate all instruction caches to PoU (Inner Shareable)
    // Reference: ARM DDI 0487, D4.4.6
    ic      ialluis
    dsb     sy
    isb

    // Step 6: Memory barriers to ensure all operations complete
    dsb     sy                  // Data Synchronization Barrier (system)
    isb                         // Instruction Synchronization Barrier

    // Step 7: Restore DTB pointer to x0 and jump to new kernel
    // The new kernel expects:
    //   x0 = DTB pointer (ARM boot protocol)
    //   PC = kernel entry point
    mov     x0, x19             // x0 = DTB pointer
    br      x20                 // Jump to new kernel (branch to register)

    // Should never return
halt:
    wfe
    b       halt

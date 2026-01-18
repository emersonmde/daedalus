ENTRY(_start)

SECTIONS
{
    /* Pi firmware loads kernel8.img at 0x80000 */
    . = 0x80000;

    /* Boot code section - must come first */
    .text.boot : {
        KEEP(*(.text.boot))
    }

    /* Exception vectors - MUST be aligned to 2048 bytes (0x800) */
    .text.exceptions : ALIGN(0x800) {
        KEEP(*(.text.exceptions))
    }

    /* Main code section */
    .text : {
        *(.text .text.*)
    }

    /* Read-only data */
    .rodata : {
        *(.rodata .rodata.*)
    }

    /* Initialized data */
    .data : {
        *(.data .data.*)
    }

    /* BSS (zero-initialized data) */
    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /* Heap for dynamic memory allocation */
    /* Reserve 64MB for heap after BSS (Pi 4 has 1-8 GB RAM) */
    . = ALIGN(16);
    __heap_start = .;
    . += 0x4000000;
    __heap_end = .;

    /* Stack grows downward from high memory */
    /* Reserve 2MB for stack after heap */
    . = ALIGN(16);
    . += 0x200000;
    _stack_start = .;

    /*
     * Network Kernel Staging Areas (NOT defined in linker script)
     * ─────────────────────────────────────────────────────────────
     * Staging Area A: 0x01000000 - 0x02000000 (16 MB)
     * Staging Area B: 0x02000000 - 0x03000000 (16 MB)
     *
     * These regions are reserved for kexec-based network boot with
     * ping-pong staging for rapid iteration:
     *
     * Bootstrap (SD card) → Stage at A → kexec to A
     * Network kernel at A → Stage at B → kexec to B
     * Network kernel at B → Stage at A → kexec to A
     * (repeat: fetch v6 → kexec → fetch v7 → kexec → ...)
     *
     * This avoids overwriting the running kernel during development,
     * enabling rapid iteration without SD card swaps.
     *
     * These regions are deliberately NOT in the linker script because:
     * 1. They're only used transiently during kexec
     * 2. Overlaps are prevented by runtime validation
     * 3. Bootstrap kernel ends ~0x04280000, so no conflict
     *
     * See: src/arch/aarch64/kexec.rs for layout constants and ping-pong logic
     *      src/boot_mode.rs for boot mode detection
     */

    /* Discard unnecessary sections */
    /DISCARD/ : {
        *(.comment)
        *(.gnu*)
        *(.note*)
        *(.eh_frame*)
    }
}

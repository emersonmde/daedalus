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
     * Network staging areas (not in linker script - used transiently by kexec):
     *   0x01000000 - 0x02000000 (16 MB) - Area A
     *   0x02000000 - 0x03000000 (16 MB) - Area B
     * Ping-pong between A/B to avoid self-overwrite during network deployment.
     * See src/arch/aarch64/kexec.rs for layout constants.
     */

    /* Discard unnecessary sections */
    /DISCARD/ : {
        *(.comment)
        *(.gnu*)
        *(.note*)
        *(.eh_frame*)
    }
}

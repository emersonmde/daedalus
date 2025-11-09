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

    /* Stack grows downward from high memory */
    /* Reserve 2MB for stack after BSS */
    . = ALIGN(16);
    . += 0x200000;
    _stack_start = .;

    /* Discard unnecessary sections */
    /DISCARD/ : {
        *(.comment)
        *(.gnu*)
        *(.note*)
        *(.eh_frame*)
    }
}

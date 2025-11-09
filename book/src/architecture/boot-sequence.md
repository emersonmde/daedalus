# Boot Sequence

Complete boot flow from firmware to Rust kernel.

## Overview

```
Pi 4 Firmware → Assembly Stub → Rust Entry → Kernel Main
  (EL2/EL1)      (boot.s)         (_start_rust)   (kernel_main)
```

## Stage 1: Firmware

The Raspberry Pi 4 firmware (start4.elf) performs initial hardware setup:

1. Initializes CPU cores, memory, and basic peripherals
2. Loads `kernel8.img` from SD card FAT partition
3. Copies kernel to physical address `0x00080000`
4. Jumps to `_start` (first instruction in kernel)

**State at firmware handoff:**
- MMU disabled
- Caches disabled
- Interrupts masked (DAIF bits set)
- Stack pointer undefined
- Exception level: EL2 (QEMU) or EL1 (real hardware)

**IMPORTANT**: QEMU boots at EL2, real Pi 4 hardware boots at EL1. This affects which system registers are accessible.

## Stage 2: Assembly Stub (boot.s)

Located at `src/arch/aarch64/boot.s`, linked first via `.text.boot` section.

### Entry Point (`_start`)

```asm
.section .text.boot
.global _start
_start:
    // 1. Check core ID
    mrs x0, mpidr_el1
    and x0, x0, #0xFF        // Extract Aff0 field (core number)
    cbnz x0, park_core       // Park non-zero cores

    // 2. Set up stack
    ldr x0, =_stack_start
    mov sp, x0

    // 3. Clear BSS section
    ldr x0, =__bss_start
    ldr x1, =__bss_end
clear_bss:
    cmp x0, x1
    b.hs clear_bss_done
    str xzr, [x0], #8
    b clear_bss
clear_bss_done:

    // 4. Jump to Rust
    bl _start_rust

    // Should never return
hang:
    wfe
    b hang
```

### Core Parking

```asm
park_core:
    wfe          // Wait for event
    b park_core  // Loop forever
```

Cores 1-3 are parked in low-power mode. Future milestones will wake them for SMP support.

## Stage 3: Rust Entry (main.rs)

The `_start_rust` function in `src/main.rs`:

```rust
#[no_mangle]
pub extern "C" fn _start_rust() -> ! {
    // Initialize kernel subsystems
    daedalus::init();  // Initializes UART, exception vectors

    #[cfg(test)]
    test_main();       // Run tests if in test mode

    #[cfg(not(test))]
    daedalus::shell::run();  // Launch interactive shell

    // Never returns
    loop {
        core::hint::spin_loop();
    }
}
```

## Stage 4: Kernel Initialization (lib.rs)

The `daedalus::init()` function performs subsystem setup:

```rust
pub fn init() {
    // 1. UART already usable (firmware initialized it)
    // Our driver just takes control

    // 2. Install exception vector table
    unsafe {
        exceptions::install_vector_table();
    }

    // 3. Print boot banner
    println!("Welcome to DaedalusOS!");
    println!("Type 'help' for available commands.\n");
}
```

## Memory Layout During Boot

Defined in `linker.ld`:

```
0x00080000: .text.boot    (assembly entry point)
0x00080xxx: .text         (Rust code)
0x000xxxxx: .rodata       (read-only data)
0x000xxxxx: .data         (initialized data)
0x000xxxxx: .bss          (zero-initialized data)
0x000xxxxx: _stack_end    (stack grows down from here)
0x000xxxxx: _stack_start  (initial SP points here)
```

## Exception Level Differences

### QEMU Behavior
- Boots at EL2 (hypervisor mode)
- `ELR_EL1`, `SPSR_EL1` may be inaccessible/zero
- Use EL2 registers when needed

### Real Hardware Behavior
- Boots at EL1 (kernel mode)
- EL1 system registers fully accessible
- Exception handling works as documented

This discrepancy is documented as tech debt in exception handling code.

## Verification

Expected serial output after successful boot:
```
Welcome to DaedalusOS!
Type 'help' for available commands.

daedalus>
```

## Code References

- Assembly stub: `src/arch/aarch64/boot.s`
- Rust entry: `src/main.rs` (`_start_rust`)
- Init function: `src/lib.rs` (`init`)
- Linker script: `linker.ld`

## External References

- [ARM Cortex-A72 TRM](https://developer.arm.com/documentation/100095/0003) - Section 4.1 (reset behavior)
- [ARMv8-A ISA](https://developer.arm.com/documentation/ddi0602/2024-12) - Section D1.2 (exception levels)

## Related Documentation

- [Exception Handling](exceptions.md) - Vector table installation
- [Linker Script](linker-script.md) - Memory layout and symbols
- [Memory Map](../hardware/memory-map.md) - Physical address space

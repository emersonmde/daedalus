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
- MMU disabled (identity addressing, no virtual memory)
- Data and instruction caches disabled
- Interrupts masked (DAIF bits set - D, A, I, F all masked)
- Stack pointer undefined (must be set by boot code)
- Exception level: EL2 (QEMU) or EL1 (real hardware)
- All cores running (core 0 continues, cores 1-3 must be parked)

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

The `daedalus::init()` function performs subsystem setup in a specific order:

```rust
pub fn init() {
    // 1. Initialize MMU first (before UART or any other subsystem)
    //    - Sets up 3-level translation tables (L1, L2)
    //    - Identity maps 0-1 GB (normal memory) and 3-4 GB (MMIO)
    //    - Configures MAIR_EL1, TCR_EL1, TTBR0_EL1
    //    - Enables MMU, data cache, and instruction cache
    unsafe {
        arch::aarch64::mmu::init();
    }

    // 2. Initialize UART driver
    //    - Firmware already initialized it, we just take control
    //    - Now we can print boot messages
    drivers::uart::WRITER.lock().init();

    // 3. Print boot sequence header
    println!("DaedalusOS v{} booting...", VERSION);
    println!("[  OK  ] MMU initialized (virtual memory enabled)");

    // 4. Install exception vector table
    exceptions::init();
    println!("[  OK  ] Exception vectors installed");

    // 5. Initialize GIC-400 interrupt controller
    //    - Configure distributor and CPU interface
    //    - Enable UART0 interrupt (ID 153)
    let mut gic = drivers::gic::GIC.lock();
    gic.init();
    gic.enable_interrupt(drivers::gic::irq::UART0);
    println!("[  OK  ] GIC-400 interrupt controller initialized");

    // 6. Enable UART RX interrupts and unmask IRQs at CPU level
    drivers::uart::WRITER.lock().enable_rx_interrupt();
    enable_irqs();  // Unmasks I bit in DAIF register
    println!("[  OK  ] IRQs enabled (interrupt-driven I/O active)");

    // 7. Initialize heap allocator
    //    - 8 MB region defined in linker.ld
    //    - Simple bump allocator for String/Vec support
    unsafe {
        extern "C" {
            static __heap_start: u8;
            static __heap_end: u8;
        }
        let heap_start = &__heap_start as *const u8 as usize;
        let heap_end = &__heap_end as *const u8 as usize;
        ALLOCATOR.init(heap_start, heap_end);
    }
    println!("[  OK  ] Heap allocator initialized (8 MB)");

    // 8. Print final boot message
    println!("Boot complete. Running at EL{}.", current_el());
}
```

### Initialization Order Rationale

**Why MMU first?**
- Identity mapping (VA = PA) means all existing addresses remain valid
- Enables caching for performance boost throughout boot
- Must happen before any significant memory operations

**Why UART second?**
- Need UART working to print boot status messages
- Firmware already initialized it, we just configure our driver

**Why exceptions before interrupts?**
- Exception vectors must be installed before any interrupts can occur
- IRQ handler is part of exception vector table

**Why GIC before enabling IRQs?**
- GIC must be configured before CPU accepts interrupts
- UART interrupt must be enabled in GIC before unmasking CPU IRQs

**Why heap last?**
- Not needed for early initialization
- Requires linker symbols which are available throughout boot
- Allocations only needed for shell and runtime features

## Memory Layout During Boot

Defined in `linker.ld`:

```
0x00080000: .text.boot       (assembly entry point)
0x00080800: .text.exceptions (exception vector table, 2KB aligned)
0x00081xxx: .text            (Rust code)
0x000xxxxx: .rodata          (read-only data, string literals)
0x000xxxxx: .data            (initialized globals)
0x000xxxxx: .bss             (zero-initialized globals)
0x000xxxxx: __heap_start     (8 MB heap region)
0x00xxxxxx: __heap_end
0x00xxxxxx: (2 MB stack, grows downward)
0x00xxxxxx: _stack_start     (initial SP points here)

[Page Tables - allocated in .bss by MMU module]
L1_TABLE:       4 KB (512 entries × 8 bytes)
L2_TABLE_LOW:   4 KB (maps 0-1 GB)
L2_TABLE_MMIO:  4 KB (maps 3-4 GB)
```

**Note**: After MMU initialization, all addresses are virtual, but identity-mapped (VA = PA).

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
DaedalusOS v0.1.0 booting...

[  OK  ] MMU initialized (virtual memory enabled)
[  OK  ] Exception vectors installed
[  OK  ] GIC-400 interrupt controller initialized
[  OK  ] IRQs enabled (interrupt-driven I/O active)
[  OK  ] Heap allocator initialized (8 MB)

Boot complete. Running at EL2.

Welcome to DaedalusOS!
Type 'help' for available commands.

daedalus>
```

### Boot Time

On QEMU with KVM acceleration, boot typically completes in <100ms. Real hardware boot time depends on firmware initialization (~1-2 seconds before kernel starts).

## Code References

- Assembly stub: `src/arch/aarch64/boot.s`
- Rust entry: `src/main.rs` (`_start_rust`)
- Init function: `src/lib.rs` (`init`)
- Linker script: `linker.ld`

## External References

- [ARM Cortex-A72 TRM](https://developer.arm.com/documentation/100095/0003) - Section 4.1 (reset behavior)
- [ARMv8-A ISA](https://developer.arm.com/documentation/ddi0602/2024-12) - Section D1.2 (exception levels)

## Boot Sequence Diagram

```
Firmware (start4.elf)
  ↓
Load kernel8.img @ 0x80000
  ↓
Jump to _start (boot.s)
  ├─→ Core 1-3: park in WFE loop
  └─→ Core 0: continue
       ↓
    Set SP = _stack_start
       ↓
    Clear BSS section
       ↓
    Jump to _start_rust (main.rs)
       ↓
    Call daedalus::init()
       ├─→ MMU init (enable virtual memory + caches)
       ├─→ UART init (take control from firmware)
       ├─→ Exception vectors (install VBAR_ELx)
       ├─→ GIC init (configure interrupt controller)
       ├─→ IRQ enable (unmask interrupts)
       └─→ Heap init (setup allocator)
       ↓
    Launch shell (shell::run())
       ↓
    Read-Eval-Print Loop
```

## Performance Optimizations

After MMU initialization:
- **Data cache enabled**: ~100x faster memory access for hot data
- **Instruction cache enabled**: ~10-100x faster instruction fetch
- **TLB active**: Fast virtual-to-physical address translation

These optimizations make the shell responsive and enable real-time interrupt handling.

## Related Documentation

- [MMU & Paging](mmu-paging.md) - Virtual memory setup details
- [Exception Handling](exceptions.md) - Vector table installation
- [Linker Script](linker-script.md) - Memory layout and symbols
- [Memory Map](../hardware/memory-map.md) - Physical address space
- [GIC Interrupts](../hardware/gic.md) - Interrupt controller setup

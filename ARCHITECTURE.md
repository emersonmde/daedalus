# DaedalusOS Architecture & Multi-Arch Plan

> Status: design in progress (updated 2025-11-08)  
> Targets: `x86_64` (Philipp Oppermann tutorial), `aarch64` (Raspberry Pi 4 Model B, Cortex-A72)

This document captures the technical direction for evolving DaedalusOS from a tutorial-aligned x86_64 hobby kernel into a portable codebase that also boots on Raspberry Pi 4 hardware and QEMU. It consolidates lessons from:

- **DaedalusOS (this repo)** — current Rust kernel built along [Philipp Oppermann’s “Writing an OS in Rust”](https://os.phil-opp.com/) series.
- **armos** — private Raspberry Pi 4 proof-of-concept in C/assembly. Critical magic values and memory maps from that effort are documented here so the knowledge is no longer siloed.
- **xyos** — legacy C kernel published at <https://github.com/emersonmde/xyos>, which demonstrates a structured arch/driver split, terminal stack, keyboard input, and shell loop.

Anyone cloning this repository should be able to follow the plan below without access to the other codebases.

---

## 1. Vision & Design Principles

1. **Stay Tutorial-Compatible**  
   Keep the x86_64 path aligned with the Phil Opp chapters. We should be able to continue stepping through the blog posts, verify code against his guidance, and run the provided test suites unchanged on the PC target.

2. **First-Class Raspberry Pi 4 Support**  
   Add an AArch64 build that produces a Pi-ready `kernel8.img`, boots via the Pi firmware or QEMU’s `-M raspi4b` machine, and eventually feature-parity with the tutorial milestones (exceptions, paging, heap, etc.).

3. **Shared Kernel Logic, Pluggable Arch Layers**  
   Treat architecture-specific code as thin shims (boot flow, MMIO drivers, interrupt glue) plugging into a common “kernel core” crate. This mirrors the structure of `xyos` but leverages Rust features (`cfg`, traits, modules) rather than Makefile logic.

4. **Borrow Proven Ideas**  
   - From *armos*: boot sequence, linker layout, UART/console bring-up, and QEMU/GDB recipes; carry over the exact register addresses (documented in §5).
   - From *xyos*: directory layout (`arch/`, `drivers/`, shell), VGA terminal handling, PS/2 keyboard parsing, REPL semantics.
   - From *Phil Opp*: memory-safe abstractions, deferred initialization via `lazy_static`, guard against UB, and the testing discipline.

5. **Explicit Knowledge Gaps**  
   Every assumption that lacks primary documentation must be called out for future validation. If we don’t know how, say, the GIC-400 distributor behaves under QEMU, the doc must state what still needs research and potential workarounds.

---

## 2. Target Architecture Overview

| Aspect | x86_64 (Current) | Raspberry Pi 4 / AArch64 (Planned) |
| --- | --- | --- |
| Toolchain | `nightly-x86_64-unknown-none`, `bootimage` runner | `nightly-aarch64-unknown-none` (no std), custom linker |
| Boot flow | Bootloader crate loads ELF, jumps to `_start` | Pi firmware loads `kernel8.img` at `0x0008_0000`, executes `_start` |
| Console | VGA text buffer (0xB8000) | PL011 UART @ `0xFE20_1000` (see §5) routed to QEMU serial |
| Interrupt controller | PIC/APIC (future tutorial steps) | GIC-400 (need research) |
| Memory map | Provided by bootloader; identity map low 1 MiB | Custom page tables mapping RAM + peripherals (0xFE00_0000 block) |

---

## 3. Workspace & Code Organization

### 3.1 Cargo Workspace Layout

```
daedalus-os/
├─ Cargo.toml               # workspace definition
├─ ARCHITECTURE.md          # (this document)
├─ kernels/
│  ├─ core/                 # arch-independent kernel logic (panic, scheduler, shell, traits)
│  ├─ arch-x86_64/          # Phil Opp tutorial path (currently existing code)
│  └─ arch-aarch64/         # Raspberry Pi 4 implementation
├─ xtask/                   # optional automation crate for builds/tests
└─ tools/                   # scripts, linker files, docs
```

Key points:

- `kernels/core` exposes traits (console, timer, interrupt controller), shared data structures, shell logic, allocator scaffolding, etc.
- `kernels/arch-*` provide `pub fn init()` entry points satisfying the traits and hooking up the boot flow.
- Top-level `src/main.rs` becomes a thin shim selecting the right arch module via `cfg`.

### 3.2 Target Specifications

- **x86_64**: keep `x86_64-daedalus-os.json` as-is. Boot via `bootimage` until we intentionally replace it.
- **aarch64**: add `aarch64-daedalus-os.json` with:
  - `llvm-target = "aarch64-unknown-none"`
  - `features = "+strict-align,+neon"`
  - `disable-redzone = true` (match Rust bare-metal best practices)
  - `panic-strategy = "abort"`
- Provide `.cargo/config.toml` entries to map custom targets to runner commands (`bootimage runner` for x86, `cargo xtask run-pi` for Pi).

---

## 4. Boot Flow Designs

### 4.1 x86_64 (Status Quo)

- Bootloader crate sets up paging and stack, jumps to `_start`.
- `_start` prints “Hello, world” via VGA driver and loops.
- Next tutorial steps (IDT, paging, heap, etc.) integrate naturally; no immediate changes.

### 4.2 Raspberry Pi 4 (Derived from armos)

1. **Firmware**: `kernel8.img` is loaded at physical `0x0008_0000`. We must ensure our linker places `.text.boot` at that address.
2. **Assembly stub** (ported from `armos/src/boot.S`):
   - Read `MPIDR_EL1` to allow only core 0 to continue; secondary cores park in `wfe`.
   - Zero `.bss` using `__bss_start`/`__bss_end`.
   - Set stack pointer near `_start` (temporary) before calling Rust `_start_rust`.
3. **Rust entry**:
   - Initialize per-core stacks (future multi-core work).
   - Set up UART (PL011) so early `println!` works.
   - Optionally set exception vector base (`VBAR_EL1`) once vector table exists.
4. **Linker script** (`tools/aarch64/kernel.ld`):
   ```ld
   ENTRY(_start)
   SECTIONS {
       . = 0x00080000;
       .text.boot : { *(.text.boot) }
       .text      : { *(.text*) }
       .rodata    : { *(.rodata*) }
       .data      : { *(.data*) }
       __bss_start = .;
       .bss       : { *(.bss*) *(COMMON) }
       __bss_end = .;
   }
   ```
   - Matches the proven layout from armos’s `kernel.ld`.

5. **QEMU launch** (from armos Makefile):
   ```
   qemu-system-aarch64 \
     -M raspi4b \
     -cpu cortex-a72 \
     -smp 4 \
     -kernel build/kernel.elf \
     -serial stdio
   ```
6. **GDB recipe**: start QEMU with `-s -S` and attach `aarch64-elf-gdb` (already scripted in armos; we will mirror this via `xtask gdb-pi`).

---

## 5. Raspberry Pi 4 Magic Values (from armos)

These addresses cost real lab time to verify. They must stay documented here for future contributors.

| Block | Address | Notes |
| --- | --- | --- |
| **Firmware entry** | `0x0008_0000` | Start of `kernel8.img` in physical RAM. `_start` must live here. |
| **Peripherals base** | `0xFE00_0000` | Low-peripheral MMIO window on Pi 4 (BCM2711). All device offsets are relative to this. |
| **GPIO registers** | `GPFSEL1 = base + 0x200004`<br>`GPPUD = base + 0x200094`<br>`GPPUDCLK0 = base + 0x200098` | Needed to mux pins 14/15 into UART ALT0 function and disable pulls. |
| **PL011 UART0** | `DR = base + 0x201000`<br>`FR = base + 0x201018`<br>`IBRD = base + 0x201024`<br>`FBRD = base + 0x201028`<br>`LCRH = base + 0x20102C`<br>`CR = base + 0x201030`<br>`IMSC = base + 0x201038`<br>`ICR = base + 0x201044` | Verified with console loopback; FR bit 5 = TXFF, bit 4 = RXFE. Baud divisor for 115200 at 54 MHz clock: IBRD=29, FBRD=19. |
| **Stack init** | `_start` page (via `adrp`) used as temporary SP before Rust runtime sets real stacks. |
| **Core filtering** | `MPIDR_EL1[1:0]` used to keep only core 0 running (`and x1, x1, #3; cbz x1, setup`). |

Any future peripheral (timer, mailbox, GIC) must have its base and tested offsets appended to this table once validated.

---

## 6. HAL & Trait Abstractions

Define traits in `kernels/core/src/hal`:

- `Console`: `fn put_byte(u8)`, `fn get_byte() -> Option<u8>`, `fn flush()`.
- `InterruptController`: `fn init()`, `fn enable(irq)`, `fn disable(irq)`, `fn ack(irq)`.
- `Timer`: monotonic tick source for scheduling/tests.
- `MemoryManager`: architecture-specific paging helpers (e.g., `init_identity_map()`, `map_region()`).

Implementations:

- `arch-x86_64` uses VGA writer + legacy PIC/APIC when tutorial reaches interrupts.
- `arch-aarch64` uses PL011 console, GIC stubs (polling until implemented), and page-table builder derived from Raspberry Pi Rust tutorials.

`println!` macro should target the trait rather than a global static. Keep `lazy_static` locks per console implementation.

---

## 7. Build & Tooling Strategy

1. **Workspace commands** (managed via `xtask` crate or scripts):
   - `cargo xtask run-x86-qemu`
   - `cargo xtask run-pi-qemu`
   - `cargo xtask gdb-pi`
   - `cargo xtask build-all` (ensures both targets build before pushing).
2. **Artifacts**:
   - x86: `target/x86_64-daedalus-os/debug/bootimage-daedalus-os.bin`
   - Pi: `target/aarch64-daedalus-os/debug/kernel.elf` + `kernel8.img`
3. **Testing**:
   - Keep Phil Opp’s custom test runner for x86.
   - For Pi, rely on host-side unit tests for shared logic until we build a QEMU-based smoke test harness (future TODO).
   - **Iteration policy**: every milestone must end with reproducible build, run, and QEMU steps for each affected target. Provide the precise commands (`cargo bootimage` + `qemu-system-x86_64 …`, or `cargo build --target aarch64-daedalus-os.json` + `qemu-system-aarch64 -M raspi4b …`) and describe the expected output (e.g., serial prints `Hello from Daedalus`). If you cannot run QEMU locally, ask the operator to run those commands and report back before closing the milestone.

---

## 8. Task Breakdown

1. **Refactor to workspace**  
   - Move current code into `kernels/arch-x86_64`.  
   - Introduce `kernels/core` crate exporting traits + re-export macros.  
   - Update `Cargo.toml` accordingly.  
   - *Risk*: keep `bootimage` runner working; verify with `cargo bootimage`.

2. **Author `ARCHITECTURE.md`** ✅ (this document).

3. **Add AArch64 target spec & linker**  
   - Create `aarch64-daedalus-os.json` + `kernel.ld`.  
   - Write assembly `_start` (global_asm).  
   - Provide `build.rs` or `xtask` logic to link with `rust-lld`.

4. **Port UART/console**  
   - Implement PL011 driver in Rust using MMIO addresses above.  
   - Provide safe wrapper struct with volatile reads/writes.  
   - Integrate into `println!` flow; ensure concurrency safety.

5. **Boot “Hello, world” on Pi**  
   - Minimal `_start_rust` that initializes console and prints.  
   - Validate on QEMU with command in §4.2.

6. **Abstract tutorials**  
   - For each Phil Opp chapter starting with “VGA Text Mode”, mirror functionality:  
     - Exceptions → AArch64 vector table.  
     - Paging → Pi stage-1 MMU set-up.  
     - Heap → share allocator logic over both targets.  
   - Record divergences when hardware differs (e.g., PIC vs. GIC).

7. **Shell & Input**  
   - Port `xyos` shell to Rust.  
   - On x86, re-implement PS/2 driver.  
   - On Pi, use UART input first; note USB keyboard as future work.

8. **Future: Multi-core & Interrupts**  
   - Research GIC-400 initialization (needs documentation).  
   - Implement mailbox-based secondary-core boot for Pi.  
   - Mirror APIC handling for x86 per tutorial.

---

## 9. Known Unknowns & Research TODOs

| Topic | Status | Notes / Workaround |
| --- | --- | --- |
| **GIC-400 bring-up** | Not researched | Need Arm docs / `qemu-system-aarch64` behavior. Until then, run in polling mode without external interrupts. |
| **Timer source on Pi** | Partially known | Probably use system timer or ARM generic timer. Need reliable CNTFRQ value (54 MHz?) from firmware. |
| **USB keyboard on Pi** | Not planned | Serial console suffices; note future need for USB host stack if VGA/keyboard desired. |
| **Actual hardware boot** | Pending | QEMU path defined. For real Pi we must prepare FAT partition, config.txt enabling 64-bit mode, copy `kernel8.img`. Document once tested. |
| **Rust target availability** | Assumed | Verify `rustup target add aarch64-unknown-none`. If not, use `aarch64-unknown-none-softfloat` or build custom target spec. |

---

## 10. References

- Philipp Oppermann’s blog: <https://os.phil-opp.com/>
- Xyos repo: <https://github.com/emersonmde/xyos>
- Raspberry Pi Rust OS Tutorials (useful patterns for MMU, UART, interrupts): <https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials>
- QEMU Raspberry Pi docs: <https://qemu.readthedocs.io/en/v9.2.4/system/arm/raspi.html>

---

## 11. Next Steps Checklist

- [ ] Create Cargo workspace & move code into `kernels/`.
- [ ] Add `aarch64-daedalus-os.json` & linker script.
- [ ] Port armos boot stub into Rust, integrating documented addresses.
- [ ] Implement PL011 console + shared `println!`.
- [ ] Produce first Pi “Hello from Daedalus” via QEMU.
- [ ] Mirror upcoming Phil Opp chapters across both targets, updating this document with new hardware findings.

Please update this document whenever we validate new hardware details (e.g., timers, caches, DMA) so future contributors do not need access to private repos.

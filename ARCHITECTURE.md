# DaedalusOS Architecture — Raspberry Pi 4

> Status: design in progress (updated 2025-11-08)  
> Target hardware: Raspberry Pi 4 Model B (BCM2711, Cortex-A72)  
> Scope: single-board, single-target hobby kernel in Rust 2024

This document is the single source of truth for how DaedalusOS boots and runs on Raspberry Pi 4. We borrow ideas from Philipp Oppermann’s tutorial where they make sense, but we are no longer trying to mirror his code 1:1 or to keep an x86 build alive. All architectural decisions, addresses, and testing expectations live here so future contributors can ship features without chasing tribal knowledge.

---

## 1. Vision & Design Tenets

1. **Pi-Only, But Tutorial-Inspired** – Treat Phil Opp’s material as a catalog of good patterns (panic handling, printing, paging, etc.) and port the concepts to Pi when useful. Skip or modify anything that does not directly serve the Raspberry Pi bring-up.
2. **Document Every One-Way Door** – The 2025-11-08 decision to drop x86 support is final; re-adding another architecture would require a brand-new plan section in this document. Any future boot-flow or memory-map change must describe rationale plus rollback steps before code lands.
3. **Hardware Facts Over Assumptions** – Every magic number (MMIO base, clock divisor, linker address) must reference a datasheet or observed behavior. If we cannot verify something, record the uncertainty and a TODO for validation.
4. **Keep Build/Test Simple** – One default cargo target spec (`aarch64-daedalus-os.json`) and one QEMU invocation. Scripts (`xtask`) can wrap them later, but the base commands must remain obvious.
5. **Tight Feedback Loop** – Each milestone ends with a reproducible Pi build plus QEMU run, and the observed serial output is captured in `README.md` or the relevant PR.

### One-Way Door: Pi-Only (2025-11-08)

- **Decision**: Remove x86_64 from active support and focus exclusively on Pi 4.  
- **Impact**: Target specs, linker scripts, and runtime code assume `aarch64`. Historical x86 sources stay in git history but will rot.  
- **Reversal Plan**: If we ever need another architecture, add a new ADR-style section here with scope, testing, and shared-crate layout before touching code.

---

## 2. Hardware & Memory Overview

| Component | Value / Notes |
| --- | --- |
| CPU | Quad-core Cortex-A72 (ARMv8-A, AArch64). Run only core 0 for now by masking `MPIDR_EL1`. |
| Entry address | Pi firmware loads `kernel8.img` at physical `0x0008_0000` and jumps to `_start`. |
| DRAM | 0x0000_0000 – 0x3FFF_FFFF (1 GiB on 1 GB model). Reserve 2 MiB after the image for stacks/heap until paging exists. |
| MMIO window | 0xFE00_0000 – 0xFF80_0000. (Use `0xFE20_1000` for PL011; see below.) |
| UART (PL011) | Base `0xFE20_1000`; registers: `DR` +0x00, `FR` +0x18, `IBRD` +0x24, `FBRD` +0x28, `LCRH` +0x2C, `CR` +0x30, `IMSC` +0x38, `ICR` +0x44. Baud 115200 @ 54 MHz: `IBRD=29`, `FBRD=19`, `LCRH=0x70`, `CR=0x301`. |
| Interrupt controller | GIC-400 (distributor @ 0xFF84_1000). Not initialized yet; kernel runs in polling mode. |
| Timer | System timer (0xFE00_3000) or ARM generic timer. Research TODO. |
| GPU mailboxes | 0xFE00_B880. Useful later for property-channel queries (framebuffer, clock rate). |

Keep this table updated whenever we validate a new peripheral or magic number.

---

## 3. Boot & Memory Layout

1. **Firmware Stage** – `kernel8.img` is copied to RAM and execution begins at `_start` with MMU and caches off, SP undefined, and interrupts masked. We must set up our own stack and BSS clearing.
2. **Assembly Stub** – In AArch64 assembly:  
   - Zero `DAIF` bits we rely on (keep IRQs masked until vector table is ready).  
   - Read `MPIDR_EL1` and park any core whose `Aff0 != 0`.  
   - Point `SP` to a statically reserved stack (e.g., `_stack_start`).  
   - Jump to `_start_rust`.
3. **Rust Entry (`_start_rust`)** – Initializes `.bss`, configures the PL011 console, prints the boot banner, and eventually calls into `kernel_main` once we have a higher-level runtime.
4. **Linker Script** – `linker.ld` must place `.text.boot` at `0x0008_0000` and keep `.bss`/`.data` contiguous. Preserve space for stacks (`.stack`) and align sections to 4 KiB.
5. **Future Paging** – When enabling the MMU, identity-map the first 64 MiB, map the MMIO window as device memory, and use a higher-half layout later if desired. Document translation tables before landing the change.

---

## 4. Toolchain, Target Spec, and Artifacts

- `rust-toolchain`: nightly (Rust 2024 edition).
- `.cargo/config.toml`:
  - `target = "aarch64-daedalus.json"`.
  - Set `build-std = ["core", "compiler_builtins"]` with `compiler-builtins-mem`.
  - Use `rust-lld` with `-Clink-arg=-Tlinker.ld`.
- `cargo build --target aarch64-daedalus.json` produces `target/aarch64-daedalus/debug/daedalus` (ELF).
- `cargo objcopy --target aarch64-daedalus.json -- -O binary target/aarch64-daedalus/debug/kernel8.img` converts ELF to raw binary.
- QEMU smoke test:
  ```
  qemu-system-aarch64 \
    -M raspi4b -cpu cortex-a72 \
    -serial stdio -display none \
    -kernel target/aarch64-daedalus/debug/kernel8.img
  ```
- Expected output for the current milestone: `Welcome to Daedalus (Pi)!`. Record any change in `README.md` and `AGENTS.md`.

Dependencies: `llvm-tools` (rustup component), `rust-src` (rustup component), `cargo-binutils` (cargo install).

---

## 5. Console / UART Implementation Notes

1. **Initialization Steps**:  
   - Disable UART (`CR = 0`).  
   - Mask interrupts (`IMSC = 0`).  
   - Clear pending (`ICR = 0x7FF`).  
   - Program divisors (`IBRD`, `FBRD`) for 115200 baud.  
   - Configure line control (`LCRH = (1<<4) | (1<<5) | (1<<6)` => 8N1 + FIFO).  
   - Enable UART, TX, RX (`CR = (1<<0) | (1<<8) | (1<<9)`).
2. **Printing**: Poll `FR` bit 5 (`TXFF`) before writing to `DR`.  
3. **Reading**: Poll `FR` bit 4 (`RXFE`) before reading `DR`; convert CRLF pairs when echoing.  
4. **Synchronization**: Wrap the UART in a `spin::Mutex` so `print!` can reuse the Phil Opp-style macros. Replace the VGA-backed writer with `pl011::Console`.
5. **Future Improvements**: Add interrupt-driven RX once the GIC bring-up completes; until then, busy-wait loops are acceptable.

---

## 6. Testing & Verification

- **Build**: `cargo build --target aarch64-daedalus-os.json`.  
- **QEMU**: command above; expect the welcome string on the serial console.  
- **Hardware** (when ready): copy `kernel8.img` to the Pi’s FAT boot partition alongside `config.txt` with:  
  ```
  enable_uart=1
  arm_64bit=1
  kernel=kernel8.img
  ```
  Capture UART output via USB serial adapter at 115200 8N1.
- **Logging Policy**: Every milestone documents the exact output we expect (e.g., `Hello from Daedalus`), plus any deviations seen during testing. If you cannot run QEMU locally, request the operator to run the command and report the output before closing the task.

---

## 7. Testing

### Current Test Setup (2025-11-08)

- **Test Framework**: Custom test framework using Rust's `custom_test_frameworks` feature
- **Test Execution**: `cargo test` builds test binary and runs it in QEMU
- **Test Runner**: `qemu-runner.sh` converts ELF to binary and launches QEMU with semihosting
- **Exit Mechanism**: ARM semihosting (HLT #0xF000) with proper parameter block for ADP_Stopped_ApplicationExit
- **Exit Codes**: Status 0 on success, status 1 on failure (properly communicated to host)

### Running Tests

```bash
cargo test
```

Expected output shows test names followed by `[ok]`:
```
Running 2 tests
daedalus::test_println...    test_println output
[ok]
daedalus::trivial_assertion...    [ok]
```

### Adding New Tests

Use the `#[test_case]` attribute on functions:

```rust
#[test_case]
fn test_something() {
    assert_eq!(2 + 2, 4);
}
```

---

## 8. Roadmap & Open Questions

1. **Boot Stub** – ✅ COMPLETE - AArch64 assembly entry, BSS zeroing, PL011 console working
2. **Testing Infrastructure** – ✅ COMPLETE - Custom test framework with QEMU integration
3. **Exception Vectors** – Implement EL1 exception table (sync/IRQ/FIQ/SError) and basic handlers (panic on unexpected traps)
4. **Timer Selection** – Decide between system timer vs. generic timer, document CNTFRQ, and expose a ticking API for delays/tests
5. **Memory Management** – Design the first identity-mapped page tables and enable the MMU; record cache/TLB requirements here
6. **Allocator & Heap** – Port a simple bump allocator (tutorial-inspired) but tuned for Pi memory layout
7. **Device IO** – Add mailbox property interface for querying board serial/clock; plan for framebuffer init if we want graphical output
8. **Future Multi-Core** – Research GIC-400 bring-up and mailbox-based secondary-core start. Only pursue after single-core kernel is stable

### Research TODOs

| Topic | Status | Notes |
| --- | --- | --- |
| GIC-400 init | Not started | Need Arm ARM + Raspberry Pi docs; until then, keep IRQs masked and poll devices. |
| Timer source | Partially known | Determine reliable CNTFRQ. Raspberry Pi firmware typically reports 54 MHz but must confirm via mailbox or `cntfrq_el0`. |
| Cache maintenance | Not started | Document which barriers are required before touching MMIO or enabling the MMU. |
| USB / Keyboard | Deferred | Serial console suffices. USB host stack can wait until after basic multitasking. |

---

## 9. Documentation Hygiene

After every milestone (build + QEMU validation), update:

1. `README.md` – exact commands and expected serial output.
2. `AGENTS.md` – any new process requirements, especially decisions that feel like one-way doors.
3. `ARCHITECTURE.md` – new peripherals, address maps, or behavioral insights.

Failure to update these documents blocks the milestone from being "done."

---

This document should stay living—edit it whenever new facts emerge or when we complete roadmap items. Keeping it current is mandatory standard work.


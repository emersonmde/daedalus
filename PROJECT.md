# DaedalusOS Project Guide

> Status: Active development (updated 2025-11-09)
> Target hardware: Raspberry Pi 4 Model B (BCM2711, Cortex-A72)
> Scope: Single-board, single-target hobby OS kernel in Rust 2024

This document is the comprehensive guide to DaedalusOS: project goals, architecture, hardware details, roadmap, and development practices. We borrow ideas from Philipp Oppermann's tutorial where they make sense, but we are no longer trying to mirror his code 1:1 or to keep an x86 build alive. All architectural decisions, memory addresses, testing expectations, and future plans live here.

---

## 1. Vision & Design Tenets

1. **Pi-Only, But Tutorial-Inspired** – Treat Phil Opp's material as a catalog of good patterns (panic handling, printing, paging, etc.) and port the concepts to Pi when useful. Skip or modify anything that does not directly serve the Raspberry Pi bring-up.
2. **Document Every One-Way Door** – The 2025-11-08 decision to drop x86 support is final; re-adding another architecture would require a brand-new plan section in this document. Any future boot-flow or memory-map change must describe rationale plus rollback steps before code lands.
3. **Hardware Facts Over Assumptions** – Every magic number (MMIO base, clock divisor, linker address) must reference a datasheet or observed behavior. If we cannot verify something, record the uncertainty and a TODO for validation.
4. **Keep Build/Test Simple** – One default cargo target spec (`aarch64-daedalus.json`) and one QEMU invocation. Scripts (`xtask`) can wrap them later, but the base commands must remain obvious.
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
| DRAM | 0x0000_0000 – 0x3FFF_FFFF (1 GiB on 1 GB model). Reserve 2 MiB after the image for stacks/heap until paging exists. |
| MMIO window | 0xFE00_0000 – 0xFF80_0000. (Use `0xFE20_1000` for PL011; see below.) |
| UART (PL011) | Base `0xFE20_1000`; registers: `DR` +0x00, `FR` +0x18, `IBRD` +0x24, `FBRD` +0x28, `LCRH` +0x2C, `CR` +0x30, `IMSC` +0x38, `ICR` +0x44. Baud 115200 @ 54 MHz: `IBRD=29`, `FBRD=19`, `LCRH=0x70`, `CR=0x301`. |
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
4. **Linker Script** – `linker.ld` must place `.text.boot` at `0x0008_0000` and keep `.bss`/`.data` contiguous. Preserve space for stacks (`.stack`) and align sections to 4 KiB.
5. **Future Paging** – When enabling the MMU, identity-map the first 64 MiB, map the MMIO window as device memory, and use a higher-half layout later if desired. Document translation tables before landing the change.

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
- Expected output for the current milestone: `Welcome to Daedalus OS!`. Record any change in `README.md` and `AGENTS.md`.

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
4. **Synchronization**: Wrap the UART in a `spin::Mutex` so `print!` can reuse the Phil Opp-style macros. The mutex provides interior mutability and future-proofs for interrupts/multi-core.
5. **Future Improvements**: Add interrupt-driven RX once the GIC bring-up completes; until then, busy-wait loops are acceptable.

---

## 6. Testing & Verification

- **Build**: `cargo build`.
- **QEMU**: command above; expect the welcome string on the serial console.
- **Hardware** (when ready): copy `kernel8.img` to the Pi's FAT boot partition alongside `config.txt` with:
  ```
  enable_uart=1
  arm_64bit=1
  kernel=kernel8.img
  ```
  Capture UART output via USB serial adapter at 115200 8N1.
- **Logging Policy**: Every milestone documents the exact output we expect (e.g., `Welcome to Daedalus OS!`), plus any deviations seen during testing. If you cannot run QEMU locally, request the operator to run the command and report the output before closing the task.

---

## 7. Testing Infrastructure

### Current Test Setup (2025-11-08)

- **Test Framework**: Custom test framework using Rust's `custom_test_frameworks` feature
- **Test Execution**: `cargo test` builds test binary and runs it in QEMU
- **Test Runner**: `qemu-runner.sh` converts ELF to binary and launches QEMU with semihosting
- **Exit Mechanism**: ARM semihosting (HLT #0xF000) with proper parameter block for ADP_Stopped_ApplicationExit
- **Exit Codes**: Status 0 on success, status 1 on failure (properly communicated to host)
- **Coverage**: 25 tests covering kernel init, UART driver, print macros, formatting, shell parsing, exception vectors, edge cases

### Running Tests

```bash
cargo test
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

## 8. Roadmap

### Project Goals
- **Primary**: Learning project for OS internals, low-level hardware, and bare-metal Rust on ARM
- **Target Hardware**: Raspberry Pi 4/5 (focus on Pi 4 for now)
- **End Vision**: Minimal useful OS with shell, networking, GPIO - capable of making HTTP requests and controlling hardware
- **Development Model**: Incremental weekend projects, each milestone delivers something tangible

### Completed Milestones ✅

1. **Boot & Console** - AArch64 assembly entry, core parking, BSS zeroing, stack setup, PL011 UART driver with TX
2. **Testing Infrastructure** - Custom test framework with QEMU integration, proper exit codes
3. **UART Input** - Polling-based RX implementation, character echo, backspace/line editing support (Ctrl-U, Ctrl-C)
4. **Command Parser** - Line buffering, command/argument splitting, ASCII input handling
5. **Shell Loop** - Interactive REPL with prompt, built-in commands (help, echo, clear, version, meminfo), error handling
6. **Exception Vectors** - Exception table (16 vectors), context save/restore, ESR/ELR/FAR decoding, register dump on panic
   - **Tech Debt**: Currently runs at EL2 (QEMU default), assembly hardcodes EL1 register saves (ELR_EL1/SPSR_EL1)
   - **Impact**: ELR/SPSR/FAR show as zero in exception dumps (saved registers are correct, system registers are wrong EL)
   - **Future**: Either drop to EL1 during boot, or make exception assembly EL-agnostic

### Phase 1: Interactive Shell ✅ COMPLETE

**Goal**: Build a usable REPL that runs in QEMU, foundation for all future features

All Phase 1 milestones completed! The kernel now boots into an interactive shell with:
- Polling-based UART input (read_byte) with proper flag checking (FR bit 4)
- Line editing: backspace, Ctrl-U (clear line), Ctrl-C (cancel)
- Command parser with argument splitting
- Built-in commands: help, echo, clear (ANSI escape), version, meminfo (placeholder)
- Full REPL loop with `daedalus>` prompt

**Testing**: Run `cargo run` and interact with shell. All 19 existing tests still pass.

**Future Enhancement**: Add command history and arrow key support when heap allocator is available (Phase 2)

### Phase 2: Memory & Interrupts 🧠

**Goal**: Foundation for dynamic allocation, interrupt-driven I/O, and time-based operations

**Milestone #7 (Exception Vectors) Complete!** The kernel now has:
- 16-entry exception vector table (4 exception types × 4 exception levels) in assembly
- Context save/restore macros (all GPRs + ELR + SPSR)
- Exception handlers in Rust with full register dump (x0-x30)
- ESR decoding with 40+ exception class descriptions
- FAR (fault address) reporting
- VBAR installation at kernel init (EL1 or EL2 based on current EL)
- Test command: `exception` in shell triggers BRK instruction

**Testing**: Run `cargo test` (25 tests pass). In shell, type `exception` to see:
```
Exception Class: 0x3c (BRK instruction (AArch64))
Registers: x0-x30 with actual values
```

**Known Limitation (Tech Debt)**:
- QEMU boots at EL2, real Pi 4 hardware may boot at EL1
- Exception assembly hardcodes `elr_el1`/`spsr_el1` saves (should be EL-specific)
- Current workaround: Rust code detects EL and sets VBAR_EL2 when at EL2
- Impact: ELR/SPSR show as zero in dumps (wrong EL register read), but GPRs are correct
- Future fix options:
  1. Drop to EL1 during boot (requires proper EL2→EL1 transition with HCR_EL2 setup)
  2. Make exception assembly EL-agnostic (check CurrentEL, save appropriate registers)
  3. Accept EL2 as standard for QEMU, add conditional assembly for Pi hardware

7. **Heap Allocator**
   - Implement simple bump allocator
   - Integrate with Rust's `alloc` crate
   - Enable `Vec`, `String`, `Box`, `BTreeMap`
   - Test: Allocate and free memory, use dynamic collections
   - Deliverable: Shell can use dynamic strings, store command history in Vec

8. **Exception Vectors** ✅ COMPLETE

9. **ARM Generic Timer**
   - Configure and enable ARM Generic Timer
   - Read CNTFRQ_EL0 and CNTPCT_EL0
   - Implement timer interrupts
   - Add delay functions (`sleep`, `usleep`)
   - Test: Timer ticks, delays work accurately
   - Deliverable: Shell command `sleep 5` waits 5 seconds, periodic ticks

10. **Memory Management (MMU)**
   - Design initial page table layout (identity-map + higher-half)
   - Enable MMU with appropriate cache/barrier setup
   - Identity-map first 64 MiB, map MMIO as device memory
   - Document cache maintenance and barriers
   - Test: Kernel runs with MMU enabled, UART still works
   - Deliverable: Virtual memory enabled, foundation for process isolation

### Phase 3: Multi-Core 🔄

**Goal**: Leverage all 4 cores of the Pi 4, demonstrate what was too complex on x86

10. **GIC-400 Interrupt Controller**
    - Initialize GIC-400 (distributor + CPU interface)
    - Route interrupts to specific cores
    - Document the GIC quirks and configuration
    - Test: Receive interrupts on core 0
    - Deliverable: Proper interrupt controller instead of polling

11. **Secondary Core Bringup**
    - Wake cores 1-3 from park loop
    - Set up per-core stacks
    - Per-core entry point with initialization
    - Test: All 4 cores print "Core N alive"
    - Deliverable: All cores running

12. **Inter-Core Communication**
    - Spinlocks with ARM exclusive access instructions
    - Inter-processor interrupts (IPI) via GIC
    - Simple message passing between cores
    - Test: Core 0 sends message to core 2
    - Deliverable: Cores can coordinate work

13. **Basic Scheduler**
    - Round-robin task scheduler
    - Task structure (context, stack, state)
    - Context switching
    - Schedule tasks across cores
    - Test: Run 4 tasks concurrently (one per core)
    - Deliverable: Shell command `ps` shows running tasks

### Phase 4: Network Stack 🌐

**Goal**: Make HTTP requests from the Pi - requires real hardware

14. **Ethernet Driver (GENET)**
    - BCM2711 GENET Ethernet controller driver
    - DMA ring buffers for RX/TX
    - MAC address configuration
    - Test: Send/receive raw Ethernet frames
    - Deliverable: Can see packets on network

15. **TCP/IP Stack**
    - Integrate `smoltcp` crate (no_std TCP/IP)
    - ARP, IPv4, ICMP (ping)
    - UDP sockets
    - Test: Ping from host to Pi, ping from Pi to host
    - Deliverable: Shell command `ping 8.8.8.8` works

16. **TCP & HTTP Client**
    - TCP connection establishment
    - HTTP client implementation
    - DNS resolution (or hardcode IPs initially)
    - Test: `curl http://example.com`
    - Deliverable: Make HTTP GET request from shell

### Phase 5: Userspace & Beyond 👤

**Goal**: Run user programs, basic process isolation

17. **EL0 User Mode**
    - Switch to EL0 for user programs
    - System call interface (SVC instruction)
    - Minimal syscalls: write, exit, sleep
    - Test: User program calls syscalls
    - Deliverable: Kernel/user separation

18. **ELF Loader**
    - Parse ELF headers
    - Load segments into memory
    - Set up user stack
    - Jump to entry point
    - Test: Load and run simple "Hello World" binary
    - Deliverable: Can execute pre-compiled programs

19. **Process Management**
    - Process table
    - fork/exec primitives (simplified)
    - Basic scheduling between processes
    - Test: Run multiple user programs
    - Deliverable: Multi-process environment

### Optional/Future Features

- **GPIO** - Simple MMIO driver for LED/sensor control (requires hardware testing)
- **USB HID** - Keyboard/mouse support (very complex)
- **Framebuffer** - Graphics output via mailbox interface
- **SD Card** - File system support
- **WiFi** - Wireless networking (very complex)

### Research & Documentation Strategy

As each driver is implemented, document:
- Datasheet references with section numbers
- Register layouts and magic numbers with explanations
- Known contradictions between ARM/Broadcom/Pi documentation
- Observed behavior vs. documented behavior
- Build a clearer reference than vendor docs

### Current Focus

**Next Milestone**: Heap Allocator (#7) - Enable dynamic allocation for shell history and future features

---

## 9. Documentation Hygiene

After every milestone (build + QEMU validation), update:

1. `README.md` – exact commands and expected serial output.
2. `AGENTS.md` – any new process requirements, especially decisions that feel like one-way doors.
3. `ARCHITECTURE.md` – new peripherals, address maps, or behavioral insights.

Failure to update these documents blocks the milestone from being "done."

---

This document should stay living—edit it whenever new facts emerge or when we complete roadmap items. Keeping it current is mandatory standard work.

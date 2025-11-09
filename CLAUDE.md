# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

DaedalusOS is a bare-metal Rust kernel for Raspberry Pi 4 (AArch64). This is a learning project focused on OS internals and low-level ARM hardware. Key constraint: **Pi 4 only** - no x86, no other ARM boards. See `PROJECT.md` for the "one-way door" decision rationale.

## Essential Commands

### Build & Test Workflow
```bash
cargo build              # Build kernel ELF (uses custom target aarch64-daedalus.json)
cargo run                # Build + run in QEMU, launches interactive shell
cargo test               # Run 24 tests in QEMU with semihosting
```

Expected `cargo run` output: Interactive shell with `daedalus>` prompt and welcome message.

### Hardware Deployment (SD card)
```bash
cargo build --release
cargo objcopy --release -- -O binary kernel8.img
# Copy kernel8.img to Pi's FAT boot partition with config.txt
```

### Test-Specific Commands
- Tests use `#[test_case]` attribute (custom test framework, not std)
- Tests run in QEMU via `qemu-runner.sh` with ARM semihosting for exit
- All tests must work in bare-metal `no_std` environment (no heap unless allocator implemented)

## Architecture Overview

### Boot Flow (src/arch/aarch64/boot.s → src/main.rs)
1. **Firmware loads `kernel8.img` at 0x80000** and jumps to `_start` in boot.s
2. **Assembly stub** (boot.s):
   - Reads MPIDR_EL1 to identify core (Aff0 field)
   - Parks secondary cores (1-3) in WFE loop
   - Sets stack pointer to `_stack_start`
   - Clears BSS section
   - Jumps to `_start_rust` in main.rs
3. **Rust entry** (_start_rust):
   - Calls `daedalus::init()` to initialize UART
   - In test mode: runs test_main()
   - In normal mode: launches interactive shell via `daedalus::shell::run()`

### Module Structure
- **src/main.rs** - Binary entry point, panic handlers (separate for test/normal)
- **src/lib.rs** - Kernel library with `print!`/`println!` macros, test framework, 24 tests
- **src/drivers/uart.rs** - PL011 UART driver (TX/RX) with spin::Mutex wrapper
- **src/shell.rs** - Interactive REPL with command parsing, line editing (backspace, Ctrl-U, Ctrl-C)
- **src/qemu.rs** - QEMU-specific utilities (semihosting exit codes)
- **src/arch/aarch64/boot.s** - Assembly entry point

### Critical Build Configuration
- **Custom target**: `aarch64-daedalus.json` (bare-metal, no OS, panic=abort, disable-redzone)
- **build.rs**: Compiles boot.s with clang, creates libboot.a, links with +whole-archive
- **.cargo/config.toml**: Sets default target, build-std for core/compiler_builtins, links linker.ld, uses qemu-runner.sh
- **linker.ld**: Places .text.boot at 0x80000, defines BSS/stack symbols

### Hardware Details (Raspberry Pi 4 BCM2711)
- **UART (PL011)**: Base 0xFE201000, 115200 baud @ 54MHz (IBRD=29, FBRD=19)
  - TX: Poll FR bit 5 (TXFF) before writing DR
  - RX: Poll FR bit 4 (RXFE) before reading DR
  - Wrapped in `spin::Mutex` for safe concurrent access
- **Entry address**: Physical 0x80000 (kernel8.img loaded by firmware)
- **Memory**: 1GB DRAM at 0x00000000, MMIO window 0xFE000000-0xFF800000
- **Current mode**: Polling I/O only (no interrupts/GIC initialized yet)

### Print Macros Pattern (Phil Opp style)
- `print!()` and `println!()` defined in lib.rs
- Call `_print()` helper which locks UART writer and uses core::fmt::Write
- **Critical deadlock pattern**: Never hold UART lock while calling print! macros
  - Bad: `let writer = WRITER.lock(); ... println!(...);`
  - Good: `{ let ch = WRITER.lock().read_byte(); } println!(...);`

## Development Workflow

### Architecture Decision Protocol
**Any boot-flow, linker, or memory-map change is a one-way door.** Before changing:
1. Document proposal in `PROJECT.md` with rationale + rollback strategy
2. Reference datasheet sections or observed behavior for all magic numbers
3. If uncertain, record the uncertainty as TODO

### Testing Requirements
After every milestone:
1. Run `cargo test` and verify all tests show `[ok]`
2. Run `cargo run` and confirm shell launches with interactive prompt
3. For shell features: verify prompt, commands execute, line editing works
4. Update `README.md`, `PROJECT.md`, and `AGENTS.md` with new behavior/commands

### Coding Conventions
- Rust 2024 edition, nightly toolchain (pinned via rust-toolchain file)
- Use `rustfmt` before committing (4 spaces, no tabs)
- Document hardware intent in comments (especially registers, magic numbers)
- snake_case for files/functions, CamelCase for types

### Commit Guidelines
- Scoped descriptive messages in present tense: "Add PL011 console", "Define Pi linker script"
- Reference design decisions when changing boot/hardware: "Document PL011 base 0xFE201000"
- Include verification steps in PR: build + QEMU run output

## Current State & Roadmap

**Phase 1: Interactive Shell ✅ COMPLETE**
- Working REPL with `daedalus>` prompt
- Commands: help, echo, clear, version, meminfo (placeholder)
- Line editing: backspace, Ctrl-U (clear line), Ctrl-C (cancel)
- 24 passing tests (kernel init, UART TX/RX, print macros, shell parsing)

**Next Milestone: Phase 2 - Heap Allocator (#6)**
- Goal: Enable dynamic allocation for shell history and future features
- Will integrate Rust's `alloc` crate with simple bump allocator

See `PROJECT.md` Section 8 (Roadmap) for full phase breakdown through networking and userspace.

## Key Documentation Files
- **PROJECT.md** - Comprehensive guide: goals, architecture, hardware specs, roadmap
- **AGENTS.md** - Development workflows, coding standards, testing guidelines
- **README.md** - Quick start, build commands, project structure

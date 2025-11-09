# Roadmap

DaedalusOS development phases and milestones.

## Project Goals

- **Primary**: Learning project for OS internals and bare-metal ARM programming
- **Target**: Raspberry Pi 4 exclusively (see [ADR-001](decisions/adr-001-pi-only.md))
- **End Vision**: Minimal functional OS with shell, networking, GPIO control
- **Development**: Incremental weekend projects, each milestone delivers working feature

## Current Status

**Phase 1 Complete** ✅ - Interactive shell with exception handling
- 25 tests passing
- Working REPL with command parsing
- Exception vector table with register dumps

**Next**: Phase 2 - Heap allocator

## Phase 1: Interactive Shell ✅ COMPLETE

**Goal**: Usable REPL running in QEMU

**Completed Milestones:**
1. Boot & Console - Assembly entry, UART TX
2. Testing Infrastructure - Custom test framework with QEMU
3. UART Input - Polling RX, line editing (backspace, Ctrl-U, Ctrl-C)
4. Command Parser - Line buffering, argument splitting
5. Shell Loop - REPL with prompt, built-in commands (help, echo, clear, version, meminfo)
6. Exception Vectors - 16-entry table, context save/restore, ESR/FAR decoding

**Current Features:**
- Interactive shell prompt (`daedalus>`)
- Commands: help, echo, clear, version, meminfo, exception
- Line editing: backspace, Ctrl-U (clear line), Ctrl-C (cancel)
- Full exception handling with register dumps

## Phase 2: Memory & Interrupts 🔄 IN PROGRESS

**Goal**: Dynamic allocation and interrupt-driven I/O

**Milestone #7**: Heap Allocator
- Integrate Rust `alloc` crate
- Simple bump allocator for shell history
- Enable `String`, `Vec`, collections

**Milestone #8**: System Timer
- Configure BCM2711 system timer
- Implement delay functions
- Timing for future scheduler

**Milestone #9**: GIC-400 Setup
- Initialize interrupt controller
- Configure UART interrupts
- Interrupt-driven I/O (replace polling)

## Phase 3: Advanced Kernel Features

**Milestone #10**: MMU & Paging
- Translation tables (4 KiB pages)
- Identity map kernel and MMIO
- Memory protection

**Milestone #11**: Multi-Core
- Wake secondary cores (1-3)
- Core synchronization primitives
- Basic SMP scheduler

**Milestone #12**: Preemptive Scheduler
- Timer-based preemption
- Round-robin task switching
- Sleep/wake mechanisms

## Phase 4: Userspace & Processes

**Milestone #13**: EL0 Userspace
- Drop to EL0 for user programs
- System call interface (SVC)
- Memory isolation

**Milestone #14**: Process Management
- Process creation/termination
- Basic IPC (pipes/messages)
- Resource limits

## Phase 5: Filesystems & Storage

**Milestone #15**: FAT32 Driver
- Read FAT filesystem
- File operations (open, read, close)
- Directory traversal

**Milestone #16**: SD Card Driver
- EMMC controller initialization
- Block device interface
- Mount root filesystem

## Phase 6: Networking

**Milestone #17**: Ethernet Driver
- BCM54213PE PHY configuration
- Packet TX/RX
- MAC address handling

**Milestone #18**: TCP/IP Stack
- IP, UDP, TCP protocols
- Socket interface
- DNS client

**Milestone #19**: HTTP Client
- Simple HTTP GET/POST
- TLS (stretch goal)
- Network utilities

## Phase 7: Hardware I/O

**Milestone #20**: GPIO Driver
- Pin configuration
- Digital I/O (read/write)
- LED control

**Milestone #21**: I2C/SPI
- Bus initialization
- Device communication
- Sensor drivers

## Timeline Estimate

- **Phase 1**: ✅ Complete (2-3 months)
- **Phase 2**: 1-2 months
- **Phase 3**: 2-3 months
- **Phase 4**: 2-3 months
- **Phase 5-7**: 3-6 months

**Total**: ~12-18 months of weekend work

## Development Practices

Each milestone must:
1. Build successfully (`cargo build`)
2. Pass all tests (`cargo test`)
3. Run in QEMU (`cargo run`)
4. Update documentation
5. Create git commit with milestone tag

## Documentation Requirements

After each milestone, update:
- **README.md** - Quick start, expected output
- **Roadmap** (this file) - Mark milestone complete
- **Hardware docs** - New peripherals
- **Architecture docs** - New features

## Related Documentation

- [Introduction](intro.md) - Project overview
- [ADR-001](decisions/adr-001-pi-only.md) - Why Pi 4 only
- [Hardware Reference](hardware/memory-map.md) - Peripheral addresses

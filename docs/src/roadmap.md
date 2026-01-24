# Roadmap

DaedalusOS development phases and milestones.

## Project Goals

- **Primary**: Learning project for OS internals and bare-metal ARM programming
- **Target**: Raspberry Pi 4 exclusively (see [ADR-001](decisions/adr-001-pi-only.md))
- **End Vision**: Network-enabled device for remote GPIO control via HTTP
- **Development**: Incremental milestones, each delivers working feature
- **Learning Focus**: Hardware/driver layer (implement from scratch), protocols/algorithms (use existing `no_std` crates)

## Current Status

**Phase 4 In Progress** 🔄 - Networking Stack
**Milestone #14 Complete** ✅ - Interrupt-Driven Networking

**Network Boot System In Progress** 🔧 - Development Workflow Improvement
- Kexec foundation for hot kernel replacement (3ae70ee)
- HTTP/0.9 client + fetch-kernel command (3ee82f9)
- Ping-pong staging for iterative development (388d340)
- Code cleanup - removed AI verbosity (803e41d)
- Status: Software complete, awaiting hardware testing
- See: `~/.claude/plans/lazy-jumping-willow.md` for full implementation plan

**Current Features:**
- Working REPL with command parsing and shell history
- Exception vector table with register dumps
- 8 MB heap with bump allocator
- Full `alloc` crate support (Box, Vec, String, collections)
- System timer driver with microsecond precision delays
- GIC-400 interrupt controller with interrupt-driven UART RX
- MMU with 39-bit virtual address space (identity mapped)
- Caching enabled for performance
- GPIO driver with BCM2711 pull-up/down support
- Shell commands for GPIO pin control (mode, pull, set, get, toggle)
- GENET Ethernet controller with full TX/RX capability
- VideoCore mailbox driver for querying firmware properties
- MAC address retrieved from OTP (One-Time Programmable memory)
- **sk_buff packet buffers** with Arc reference counting (Linux-inspired)
- **Protocol handler registry** for extensible packet dispatch
- **ARP protocol handler** with socket delivery
- **Socket API** (socket, bind, sendto, recvfrom, close) with AF_PACKET support
- **Interrupt-driven packet routing** from GENET RX to sockets
- Shell commands: `eth-stats`, `netstats`, `arp-probe` (full end-to-end test)

**Next**: Milestone #15 - ARP Responder OR Network Boot Hardware Testing

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

## Phase 2: Memory & Interrupts ✅ COMPLETE

**Goal**: Dynamic allocation and interrupt-driven I/O

**Milestone #7**: Heap Allocator ✅ COMPLETE
- ✅ Integrated Rust `alloc` crate
- ✅ Simple bump allocator for shell history
- ✅ Enabled `String`, `Vec`, collections
- ✅ 8 MB heap region with proper alignment
- ✅ Memory tracking (heap_size, used, free)
- ✅ 6 allocator tests (Box, Vec, String, capacity, stats)

**Milestone #8**: System Timer ✅ COMPLETE
- ✅ Configured BCM2711 system timer (base 0xFE003000)
- ✅ Implemented delay functions (delay_us, delay_ms)
- ✅ Added timestamp and uptime tracking functions
- ✅ Added uptime shell command
- ✅ 6 timer tests (counter, delays, monotonicity)
- ✅ Comprehensive hardware documentation

**Milestone #9**: GIC-400 Setup ✅ COMPLETE
- ✅ Initialize interrupt controller
- ✅ Configure UART interrupts
- ✅ Interrupt-driven I/O (replaced polling)

**Milestone #10**: MMU & Paging ✅ COMPLETE
- ✅ 3-level translation tables with 2 MB block mappings
- ✅ Identity map kernel (1 GB normal memory)
- ✅ Identity map MMIO (device memory, non-cacheable)
- ✅ 39-bit virtual address space (512 GB)
- ✅ Memory attributes (cacheable normal, device-nGnRnE)
- ✅ Shell command (`mmu`) for debugging MMU status
- ✅ Comprehensive documentation

## Phase 3: Hardware I/O 🔄 IN PROGRESS

**Goal**: Foundation for real-world device control

**Milestone #11**: GPIO Driver ✅ COMPLETE
- ✅ Pin configuration (input/output, alt functions 0-5)
- ✅ BCM2711 pull-up/down resistor control (new register mechanism)
- ✅ Digital I/O (read/write/toggle GPIO pins)
- ✅ Shell commands: gpio-mode, gpio-pull, gpio-set, gpio-get, gpio-toggle
- ✅ Support for all 58 GPIO pins (BCM2711)
- ✅ Comprehensive hardware documentation

## Phase 4: Networking Stack 🔄 IN PROGRESS

**Goal**: Network-enabled device (the primary objective)

**Milestone #12**: Ethernet Driver Foundation ✅ COMPLETE
- ✅ GENET v5 hardware detection and register access
- ✅ MDIO protocol implementation (PHY communication)
- ✅ BCM54213PE PHY detection and identification
- ✅ Ethernet frame structures and parsing
- ✅ ARP packet structures and parsing
- ✅ Network byte order handling
- ✅ 30 protocol unit tests passing
- ✅ Comprehensive documentation (hardware, protocols, verification)
- ✅ Shell command: `eth-diag` (hardware diagnostics)

**Milestone #13**: Frame Transmission & Reception ✅ COMPLETE
- ✅ Frame TX implementation (polling mode with DMA descriptors)
- ✅ Frame RX implementation (polling with ring buffers)
- ✅ VideoCore mailbox driver for firmware communication
- ✅ MAC address queried from OTP via mailbox (real hardware MAC)
- ✅ Bus address translation (ARM physical → VideoCore bus)
- ✅ Cache-line aligned message buffers (64-byte alignment)
- ✅ Frame validation and error handling
- ✅ Shell command: `arp-probe` (comprehensive TX/RX diagnostics)

**Milestone #14**: Interrupt-Driven Networking ✅ COMPLETE
- ✅ Socket buffer (sk_buff) implementation with Arc reference counting
- ✅ Protocol handler registry for extensible packet dispatch
- ✅ ARP protocol handler with socket delivery
- ✅ Socket API: socket(), bind(), sendto(), recvfrom(), close()
- ✅ AF_PACKET sockets with EtherType-based routing
- ✅ Interrupt-driven RX handler with packet routing to sockets
- ✅ Lock-free socket RX queues (32-entry ring buffers)
- ✅ GIC interrupt enable/disable on socket bind/close
- ✅ Comprehensive statistics via `netstats` command
- ✅ Full end-to-end test with `arp-probe` diagnostic
- Note: TX completion interrupts deferred to future optimization

**Milestone #15**: ARP Responder
- ARP cache implementation with expiration
- ARP request/reply handling
- Respond to ARP requests for our IP
- Shell command: `arp-cache`

**Milestone #16**: TCP/IP Stack Integration (smoltcp)
- Integrate `smoltcp` no_std TCP/IP stack
- Implement Device trait (maps to GENET driver)
- IPv4 packet handling
- ICMP echo (ping support)
- DHCP client for IP configuration
- UDP/TCP socket support

**Milestone #17**: Application Protocols
- DNS resolver (A records)
- HTTP/1.1 client (GET/POST)
- Simple HTTP server for device control
- Shell commands: `ping`, `http-get`, `gpio-server`

## Network Boot Development System 🔧 IN PROGRESS

**Goal**: Transform development workflow from SD card swaps to network-based deployment

**Current Pain**: Build (10s) + SD write (5s) + SD swap (30s) + boot (2s) = 47s/iteration
**Target**: Build (10s) + network deploy (0.1s) + boot (2s) = 12s/iteration (4x faster)

**Implementation Status**: Software complete, awaiting hardware testing

### Completed Work (4 commits)

**Commit 3ae70ee**: Kexec Foundation
- Assembly stub for hot kernel replacement (disable MMU/IRQs, jump to new kernel)
- Memory layout: Bootstrap at 0x00080000, network staging at 0x01000000
- Boot mode detection (PC-based: SD card vs network)
- Shell command: `kexec <address>` for manual kernel jumping
- Heap reset on kexec to provide clean state for new kernel
- Tests: All validation, boot mode detection passing (104/104 tests)

**Commit 3ee82f9**: HTTP Client + Shell Integration
- HTTP/0.9 client API (placeholder, awaits smoltcp Device wrapper)
- Added smoltcp v0.11 dependency (proto-ipv4, socket-tcp, medium-ethernet)
- Shell command: `fetch-kernel` downloads from 10.42.10.100:8000
- Automatic kernel staging with ping-pong address selection
- Dev server workspace: `daedalus-dev-server` (Rust HTTP server on port 8000)
- Tests: HTTP request formatting, all existing tests passing

**Commit 388d340**: Ping-Pong Staging
- Dual staging areas: 0x01000000 (A) and 0x02000000 (B)
- Automatic address selection based on current PC (avoids self-overwrite)
- Workflow: Bootstrap→A, A→B, B→A (enables rapid iteration)
- Updated boot mode detection to recognize both staging areas
- Linker script documentation of memory layout
- Tests: Staging area validation, ping-pong logic, all passing

**Commit 803e41d**: Code Cleanup
- Removed 386 lines of AI-generated verbosity
- Condensed module/function documentation to essentials
- Removed redundant comments and safety explanations
- Professional kernel coding standards
- No behavior changes, pure documentation cleanup

### Architecture

```
Dev Machine (10.42.10.100)          Raspberry Pi 4 (10.42.10.42)
┌─────────────────────┐             ┌──────────────────────────┐
│ daedalus-dev-server │◄────────────│ fetch-kernel command     │
│ HTTP :8000          │  GET /kernel│ (downloads to staging)   │
│ Serves kernel8.img  │────────────►│ kexec 0x01000000         │
└─────────────────────┘             │ (jumps to new kernel)    │
                                    └──────────────────────────┘
```

### Workflow
1. Write initial kernel (v5) to SD card → boot
2. Shell: `fetch-kernel` → downloads v6 to 0x01000000
3. Shell: `kexec 0x01000000` → jump to v6
4. Shell: `fetch-kernel` → downloads v7 to 0x02000000 (ping-pong)
5. Shell: `kexec 0x02000000` → jump to v7
6. Like v7? Write to SD → continue with v8, v9...

### Pending Work
- **Phase 2**: HTTP client implementation (requires smoltcp Device wrapper for GENET)
- **Phase 4**: Hardware testing (fetch-kernel + kexec workflow)
- **Phase 5**: Watchdog driver for crash recovery (optional)
- **Phase 6**: Network shell backend (TCP shell access, optional)
- **Phase 7**: Automation scripts (network-deploy.sh, hardware-test.sh)

### References
- Full plan: `~/.claude/plans/lazy-jumping-willow.md`
- Commits: 3ae70ee, 3ee82f9, 388d340, 803e41d
- Files: `src/arch/aarch64/kexec.{rs,s}`, `src/boot_mode.rs`, `src/net/http.rs`
- Dev server: `daedalus-dev-server/src/main.rs`

## Phase 5: Advanced Features (Future Self-Implementation)

**Goal**: Optimizations and advanced capabilities

**Milestone #18**: DMA Controller
- DMA channel setup
- Optimize Ethernet for DMA-based transfers
- Improve SD card performance (when implemented)

**Milestone #19**: Better Allocator
- Replace bump allocator with buddy or slab allocator
- Free/reallocation support
- Fragmentation management

**Milestone #20**: Multi-Core Support
- Wake secondary cores (cores 1-3)
- Spinlocks and synchronization primitives
- Per-core data structures

**Milestone #21**: Cooperative Scheduler
- Task switching for async I/O
- Event-driven network processing
- Timer-based task scheduling

## Phase 6: Storage & Persistence (Optional)

**Goal**: Persistent storage and filesystems

**Milestone #22**: SD Card Driver
- EMMC controller initialization
- Block read/write operations
- Interrupt-driven I/O

**Milestone #23**: FAT32 Filesystem
- Parse FAT32 structures
- File operations (open, read, write, close)
- Directory traversal

## Phase 7: Advanced Hardware (Optional)

**Goal**: Additional peripherals and buses

**Milestone #24**: I2C/SPI Drivers
- Bus initialization
- Multi-device support
- Sensor integration

**Milestone #25**: USB Host Controller
- xHCI/EHCI initialization
- USB device enumeration
- Keyboard/mouse/storage support

## Phase 8: Userspace (Optional)

**Goal**: Process isolation and privilege separation

**Milestone #26**: EL0 Userspace
- Drop to EL0 for user programs
- System call interface (SVC handler)
- User/kernel memory isolation

**Milestone #27**: Process Management
- Process creation/termination
- Basic IPC mechanisms
- Resource limits and scheduling

## Development Practices

Each milestone must:
1. Pass pre-commit script with **no errors or warnings** (`./.githooks/pre-commit`)
   - This verifies: formatting, clippy, documentation, tests, and build
2. Run in QEMU (`cargo run`) for interactive verification
3. Update documentation (code docs, milestone summary, and relevant guides)

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

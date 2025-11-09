# Similar Projects & Tutorials

Learning resources and similar bare-metal Rust projects.

## Rust OS Tutorials

### Philipp Oppermann's Blog OS
[Writing an OS in Rust](https://os.phil-opp.com/)

**Target**: x86_64 architecture (different from our AArch64)

**Useful for reference:**
- **Testing framework** - Custom test harness pattern
- **Print macros** - `print!`/`println!` implementation using `fmt::Write` trait
- **Panic handling** - Separate panic handlers for test vs normal mode
- **VGA text mode concepts** - Console output patterns
- **Memory management** - Heap allocators, paging concepts

**Less relevant:**
- x86-specific code (bootloader, interrupts, APIC)
- VGA hardware specifics
- x86 page table format

**Best use**: Architecture patterns and Rust idioms, not hardware specifics.

### Rust Raspberry Pi OS Tutorials
[rust-raspberrypi-OS-tutorials](https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials)

**Target**: Raspberry Pi 3 and 4 (AArch64, same as us!)

**Useful for reference:**
- **Pi-specific initialization** - GPIO, UART, timer setup examples
- **AArch64 assembly** - Boot sequence, exception handling patterns
- **Linker scripts** - Section placement approaches for Pi
- **Driver patterns** - MMIO register access techniques
- **Testing approaches** - QEMU-based integration test examples

**Differences from DaedalusOS:**
- Uses Ruby-based build tooling (we use Cargo directly)
- Structured as progressive tutorials (we're focused on single working kernel)
- Supports multiple Pi models (we're Pi 4 only)

**Best use**: Reference implementation for Pi 4 hardware initialization.

## Embedded Rust Resources

### The Embedded Rust Book
[Embedded Rust Book](https://docs.rust-embedded.org/book/)

**Topics:**
- `#![no_std]` development
- Peripheral access crates (PAC pattern)
- Memory-mapped I/O
- Volatile operations
- Inline assembly

**Best use**: General embedded Rust patterns, not Pi-specific.

## C-based OS Development

### OSDev Wiki
[OSDev.org](https://wiki.osdev.org/)

**Useful sections:**
- **Meaty Skeleton** - Project structure inspiration
- **Memory management** - Paging, heaps, allocators
- **Filesystems** - Future milestone reference
- **Bootloaders** - Understanding boot process

**Note**: Most content is x86-focused. Use for concepts, not code.

### OSDev Wiki - ARM
[ARM-specific articles](https://wiki.osdev.org/ARM_Overview)

**Relevant topics:**
- Exception handling
- MMU setup
- Cache management
- SMP (multi-core) bringup

## Raspberry Pi 4 Bare Metal Projects

### rpi4-bare-metal by rhythm16
[GitHub: rhythm16/rpi4-bare-metal](https://github.com/rhythm16/rpi4-bare-metal)

**Target**: Raspberry Pi 4B (BCM2711, same as us!)

**Useful for reference:**
- **GIC-400 implementation** - Interrupt controller setup and handling examples
- **PL011 UART interrupts** - Interrupt-driven I/O patterns
- **Mini-UART driver** - Alternative UART implementation approach
- **BCM2711-specific initialization** - Hardware bringup sequence examples

**Best use**: Reference implementation for GIC-400 interrupt handling on Pi 4.

### rpi4os.com Tutorial Series
[Writing a "bare metal" OS for Raspberry Pi 4](https://www.rpi4os.com/)

**Target**: Raspberry Pi 4B

**Topics covered:**
- System timer interrupts
- Exception handling at EL1
- Interrupt controller setup
- Bare metal C programming patterns

**Best use**: Step-by-step tutorial for Pi 4 interrupt concepts.

### Valvers Bare Metal Programming
[Bare Metal Programming in C](https://www.valvers.com/open-software/raspberry-pi/bare-metal-programming-in-c-part-4/)

**Target**: Raspberry Pi series (includes Pi 4)

**Useful sections:**
- **Part 4: Interrupts** - GIC-400 explanation and setup
- Interrupt controller architecture
- Bare metal C patterns for Pi

**Best use**: Understanding interrupt flow and GIC-400 architecture.

**Important note**: All Pi 4 bare metal projects require `enable_gic=1` in config.txt!

## Project Comparisons

### When to Consult Each Resource

| Need | Resource | Why |
|------|----------|-----|
| Rust OS patterns | Blog OS | Architecture, testing, idioms |
| Pi 4 hardware | Rust Pi OS Tutorials, rpi4-bare-metal | Hardware initialization examples |
| ARM assembly | Rust Pi OS Tutorials | AArch64 boot/exception code patterns |
| Embedded Rust | Embedded Rust Book | `#![no_std]` patterns |
| OS concepts | OSDev Wiki | General OS knowledge |
| ARM architecture | OSDev ARM | ARM-specific OS dev |
| GIC-400 interrupts | rpi4-bare-metal, Valvers | Interrupt handling examples |

### Using Reference Implementations

1. **Understand the concept** from tutorials/docs
2. **Review similar implementations** in reference projects
3. **Study hardware specifications** from official datasheets
4. **Implement independently** for DaedalusOS constraints
5. **Document our approach** in code comments and docs

**Note**: These projects are reference implementations to learn from, not code to directly copy. Each has different design goals and constraints.

## Related Documentation

- [ARM Documentation](arm.md) - ARM architecture references
- [Raspberry Pi Documentation](raspberry-pi.md) - Pi 4 hardware specs
- [Design Decisions](../decisions/) - Why we made different choices

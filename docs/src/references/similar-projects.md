# Similar Projects & Tutorials

Learning resources and similar bare-metal Rust projects.

## Rust OS Tutorials

### Philipp Oppermann's Blog OS
[Writing an OS in Rust](https://os.phil-opp.com/)

**Target**: x86_64 architecture (different from our AArch64)

**What to borrow:**
- **Testing framework** - Custom test harness pattern (we adapted this)
- **Print macros** - `print!`/`println!` implementation using `fmt::Write` trait
- **Panic handling** - Separate panic handlers for test vs normal mode
- **VGA text mode concepts** - Adapted for our UART-based console
- **Memory management** - Heap allocators, paging (future milestones)

**What to skip:**
- x86-specific code (bootloader, interrupts, APIC)
- VGA hardware specifics
- x86 page table format

**Best use**: Architecture patterns and Rust idioms, not hardware specifics.

### Rust Raspberry Pi OS Tutorials
[rust-raspberrypi-OS-tutorials](https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials)

**Target**: Raspberry Pi 3 and 4 (AArch64, same as us!)

**What to borrow:**
- **Pi-specific initialization** - GPIO, UART, timer setup
- **AArch64 assembly** - Boot sequence, exception handling
- **Linker scripts** - Section placement for Pi
- **Driver patterns** - MMIO register access
- **Testing approaches** - QEMU-based integration tests

**Differences from DaedalusOS:**
- Uses Ruby-based build tooling (we use Cargo directly)
- Structured as progressive tutorials (we're focused on single working kernel)
- Supports multiple Pi models (we're Pi 4 only)

**Best use**: Reference for Pi 4 hardware initialization and driver implementation.

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

## Project Comparisons

### When to Consult Each Resource

| Need | Resource | Why |
|------|----------|-----|
| Rust OS patterns | Blog OS | Architecture, testing, idioms |
| Pi 4 hardware | Rust Pi OS Tutorials | Direct hardware examples |
| ARM assembly | Rust Pi OS Tutorials | AArch64 boot/exception code |
| Embedded Rust | Embedded Rust Book | `#![no_std]` patterns |
| OS concepts | OSDev Wiki | General OS knowledge |
| ARM architecture | OSDev ARM | ARM-specific OS dev |

### Code Porting Strategy

1. **Start with concept** from Blog OS or OSDev
2. **Check ARM specifics** in OSDev ARM or ARM docs
3. **Find Pi 4 implementation** in Rust Pi OS Tutorials
4. **Adapt to DaedalusOS** constraints (Pi 4 only, our structure)
5. **Document differences** in code comments and ADRs

## Related Documentation

- [ARM Documentation](arm.md) - ARM architecture references
- [Raspberry Pi Documentation](raspberry-pi.md) - Pi 4 hardware specs
- [Design Decisions](../decisions/) - Why we made different choices

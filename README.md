# DaedalusOS

DaedalusOS is my personal playground for learning low-level Rust by bringing up a tiny kernel on the Raspberry Pi 4. I'm porting ideas I like from Philipp Oppermann's blog and other hobby kernels, but the project exists purely so I can experiment, break things, and understand how the hardware works.

## Prerequisites

Install the required Rust components:

```bash
rustup component add llvm-tools
cargo install cargo-binutils
```

Ensure you have QEMU for AArch64 testing:
```bash
# On macOS:
brew install qemu

# On Linux:
sudo apt install qemu-system-aarch64
```

## Building and Running

### Quick Start

Build and run the kernel in QEMU:

```bash
cargo run
```

Expected output:
```
Welcome to DaedalusOS!
Type 'help' for available commands.

daedalus>
```

You can now interact with the shell! Try typing `help` to see available commands.

Press `Ctrl+A` then `X` to exit QEMU.

### Manual Build

Build the kernel for Raspberry Pi 4:

```bash
cargo build
```

This produces an ELF binary at `target/aarch64-daedalus/debug/daedalus`.

For release builds:

```bash
cargo build --release
```

## Testing

Run tests in QEMU:

```bash
cargo test
```

## Running on Real Hardware

For real Raspberry Pi hardware, you need to convert the ELF to a raw binary:

1. Build the kernel:
   ```bash
   cargo build --release
   ```

2. Convert to `kernel8.img`:
   ```bash
   cargo objcopy --release -- -O binary kernel8.img
   ```

3. Copy `kernel8.img` to the boot partition of an SD card

4. Add a `config.txt` file on the SD card with:
   ```
   enable_uart=1
   arm_64bit=1
   kernel=kernel8.img
   ```

5. Connect a USB serial adapter to the Pi's UART (GPIO 14/15)

6. Monitor at 115200 baud, 8N1

7. Power on the Pi

Note: The `objcopy` step is only needed for real hardware. QEMU loads the ELF directly via the `qemu-runner.sh` script.

## Project Structure

```
daedalus-os/
├── src/
│   ├── main.rs              # Binary entry point and panic handlers
│   ├── lib.rs               # Library root with print! macros and test framework
│   ├── shell.rs             # Interactive shell (REPL with built-in commands)
│   ├── drivers/
│   │   ├── mod.rs
│   │   └── uart.rs          # PL011 UART driver with TX/RX support
│   ├── qemu.rs              # QEMU utilities (semihosting, exit codes)
│   └── arch/
│       └── aarch64/
│           └── boot.s       # Boot assembly (core parking, BSS, stack)
├── linker.ld                # Linker script (entry at 0x80000)
├── aarch64-daedalus.json    # Custom bare-metal AArch64 target spec
├── build.rs                 # Compiles assembly and links it
├── qemu-runner.sh           # Converts ELF to binary and launches QEMU
└── .cargo/config.toml       # Target, linker flags, test runner
```

### Module Organization

The project follows a modular structure inspired by Phil Opp's blog_os and traditional OS development:

- **lib.rs** - Testable kernel library with public API
- **main.rs** - Minimal binary entry point
- **shell.rs** - Interactive shell with command parsing and built-in commands
- **drivers/** - Hardware device drivers (UART with polling-based I/O)
- **arch/** - Architecture-specific code (boot stub, low-level init)
- **qemu.rs** - Emulator-specific utilities (not for real hardware)

## Documentation

- **PROJECT.md** - Complete project guide: goals, architecture, hardware details, roadmap, and milestones
- **AGENTS.md** - Development workflows, coding guidelines, and contribution practices
- **README.md** (this file) - Quick start guide and basic project structure

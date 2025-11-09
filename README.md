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
Welcome to Daedalus (Pi)!
```

Press `Ctrl+C` to exit QEMU.

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
cargo test --bin daedalus
```

Tests will run and print results. Look for `[ok]` after each test name to verify they passed.

Example output:
```
Running 2 tests
daedalus::test_println...    test_println output
[ok]
daedalus::trivial_assertion...    [ok]
```

Note: QEMU exits with status 1 due to semihosting limitations, but test results are visible in the output.

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

- `src/main.rs` - Rust entry point, panic handler, and test framework
- `src/boot.s` - AArch64 assembly boot stub (core parking, BSS clearing, stack setup)
- `src/pl011.rs` - PL011 UART driver for console output
- `linker.ld` - Linker script (places kernel at 0x80000)
- `aarch64-daedalus.json` - Custom target specification for bare-metal AArch64
- `build.rs` - Build script that compiles assembly and links it into the binary
- `qemu-runner.sh` - Wrapper script that converts ELF to binary and launches QEMU
- `.cargo/config.toml` - Cargo configuration (target, linker flags, test runner)

## Documentation

See `ARCHITECTURE.md` for hardware details, memory layout, and design decisions.
See `AGENTS.md` for development workflows and guidelines.

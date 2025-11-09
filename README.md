<div align="center">

# DaedalusOS

**A bare-metal Rust kernel for Raspberry Pi 4**

[![CI](https://github.com/emersonmde/daedalus/actions/workflows/ci.yml/badge.svg)](https://github.com/emersonmde/daedalus/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Raspberry%20Pi%204-c51a4a.svg)](https://www.raspberrypi.com/products/raspberry-pi-4-model-b/)
[![AArch64](https://img.shields.io/badge/arch-AArch64-green.svg)](https://developer.arm.com/architectures/cpu-architecture/a-profile)
[![Documentation](https://img.shields.io/badge/docs-GitHub%20Pages-blue)](https://emersonmde.github.io/daedalus/)
[![Lines of Code](https://img.shields.io/tokei/lines/github/emersonmde/daedalus)](https://github.com/emersonmde/daedalus)
[![Code Size](https://img.shields.io/github/languages/code-size/emersonmde/daedalus)](https://github.com/emersonmde/daedalus)

</div>

---

DaedalusOS is my personal playground for learning low-level Rust by bringing up a tiny kernel on the Raspberry Pi 4. I'm porting ideas I like from Philipp Oppermann's blog and other hobby kernels, but the project exists purely so I can experiment, break things, and understand how the hardware works.

<div align="center">

### 🎯 Current Status: **Phase 1 Complete** - Interactive Shell with 25 Passing Tests

![Top Languages](https://github-readme-stats.vercel.app/api/top-langs/?username=emersonmde&repo=daedalus&layout=compact&theme=dark&hide_border=true)

</div>

---

## ✨ Features

<div align="center">

| 🚀 Core Features | 🔧 Hardware Support | 📚 Development |
|:---:|:---:|:---:|
| `#![no_std]` Bare Metal | PL011 UART Driver | mdBook Documentation |
| Exception Handling (EL2) | Raspberry Pi 4B (BCM2711) | 25 Integration Tests |
| Interactive Shell (REPL) | AArch64 (Cortex-A72) | GitHub Actions CI/CD |
| Custom Linker Script | QEMU 9.0+ Emulation | Rust 2024 Edition |

</div>

### 🛠️ Tech Stack

<div align="center">

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Raspberry Pi](https://img.shields.io/badge/-Raspberry_Pi-C51A4A?style=for-the-badge&logo=Raspberry-Pi)
![ARM](https://img.shields.io/badge/ARM-0091BD?style=for-the-badge&logo=arm&logoColor=white)
![GitHub Actions](https://img.shields.io/badge/github%20actions-%232671E5.svg?style=for-the-badge&logo=githubactions&logoColor=white)

</div>

## Prerequisites

### 1. Install Rust (nightly)

DaedalusOS requires Rust nightly for custom target and bare-metal features.

```bash
# Install rustup (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install nightly toolchain
rustup toolchain install nightly

# The project uses rust-toolchain file to automatically select nightly
```

### 2. Install Required Rust Components

```bash
# Add llvm-tools for objcopy
rustup component add llvm-tools --toolchain nightly

# Add rust-src for building core library
rustup component add rust-src --toolchain nightly

# Install cargo-binutils for objcopy command
cargo install cargo-binutils
```

### 3. Install QEMU for AArch64

**IMPORTANT:** DaedalusOS requires **QEMU 9.0 or newer** for Raspberry Pi 4 emulation (the `raspi4b` machine type was added in QEMU 9.0).

<details>
<summary><b>Platform-Specific QEMU Installation</b></summary>

<br>

**macOS (using Homebrew):**
```bash
brew install qemu
# Homebrew typically provides the latest version
```

**Linux (Debian/Ubuntu):**
```bash
sudo apt update
sudo apt install qemu-system-aarch64
```
**Note:** Ubuntu 22.04 LTS ships QEMU 6.2, which does **not** support raspi4b. Ubuntu 24.10+ ships QEMU 9.0+. If you're on an older Ubuntu version, you'll need to build QEMU from source:

```bash
# Install build dependencies
sudo apt-get install -y ninja-build libglib2.0-dev libpixman-1-dev

# Download and build QEMU 9.2
wget https://download.qemu.org/qemu-9.2.0.tar.xz
tar xf qemu-9.2.0.tar.xz
cd qemu-9.2.0
./configure --prefix=$HOME/qemu-install --target-list=aarch64-softmmu --enable-slirp
make -j$(nproc)
make install

# Add to PATH (add this to your ~/.bashrc or ~/.zshrc)
export PATH="$HOME/qemu-install/bin:$PATH"
```

**Linux (Fedora/RHEL):**
```bash
sudo dnf install qemu-system-aarch64
# Verify version is 9.0+: qemu-system-aarch64 --version
```

**Linux (Arch):**
```bash
sudo pacman -S qemu-system-aarch64
# Arch typically provides the latest version
```

</details>

### 4. Install Clang (for assembly compilation)

The build process uses Clang to assemble AArch64 assembly files.

**macOS:**
```bash
# Option 1: Install via Homebrew (recommended)
brew install llvm

# Option 2: Use Xcode Command Line Tools (may be older version)
xcode-select --install
```

**Linux (Debian/Ubuntu):**
```bash
sudo apt install clang
```

**Linux (Fedora/RHEL):**
```bash
sudo dnf install clang
```

**Linux (Arch):**
```bash
sudo pacman -S clang
```

### 5. Install mdBook (Optional - for documentation)

If you want to build or view the documentation locally:

```bash
cargo install mdbook
```

### 6. ✅ Verify Installation

Check that all tools are available:
```bash
rustc --version          # Should show nightly
cargo --version
qemu-system-aarch64 --version  # Should show 9.0 or newer
clang --version
cargo objcopy --version
```

**Verify QEMU supports raspi4b:**
```bash
qemu-system-aarch64 -M help | grep raspi
```
You should see `raspi4b` in the list. If not, your QEMU version is too old.

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

Note: The `objcopy` step is only needed for real hardware. QEMU loads the ELF directly via the `scripts/qemu-runner.sh` script.

## Project Structure

```
daedalus-os/
├── src/
│   ├── main.rs              # Binary entry point and panic handlers
│   ├── lib.rs               # Library root with print! macros and test framework
│   ├── shell.rs             # Interactive shell (REPL with built-in commands)
│   ├── exceptions.rs        # Exception handling (vectors, handlers, register dumps)
│   ├── drivers/
│   │   ├── mod.rs
│   │   └── uart.rs          # PL011 UART driver with TX/RX support
│   ├── qemu.rs              # QEMU utilities (semihosting, exit codes)
│   └── arch/
│       └── aarch64/
│           ├── boot.s       # Boot assembly (core parking, BSS, stack)
│           └── exceptions.s # Exception vector table (16 vectors, context save/restore)
├── linker.ld                # Linker script (entry at 0x80000)
├── aarch64-daedalus.json    # Custom bare-metal AArch64 target spec
├── build.rs                 # Compiles assembly and links it
├── scripts/
│   └── qemu-runner.sh       # Converts ELF to binary and launches QEMU
└── .cargo/config.toml       # Target, linker flags, test runner
```

### Module Organization

The project follows a modular structure inspired by Phil Opp's blog_os and traditional OS development:

- **lib.rs** - Testable kernel library with public API
- **main.rs** - Minimal binary entry point
- **shell.rs** - Interactive shell with command parsing and built-in commands
- **exceptions.rs** - Exception handling with register dumps and ESR decoding
- **drivers/** - Hardware device drivers (UART with polling-based I/O)
- **arch/** - Architecture-specific code (boot stub, exception vectors, low-level init)
- **qemu.rs** - Emulator-specific utilities (not for real hardware)

## Documentation

### Building Documentation

Build unified documentation (mdBook + cargo API docs):

```bash
./scripts/build-docs.sh
```

View locally with live reload:

```bash
mdbook serve
# Open http://localhost:3000
```

Or open the static files directly:

```bash
open book/book/index.html
```

### Documentation Files

- **[book/src/](book/src/)** - Comprehensive project documentation (hardware specs, architecture, design decisions, roadmap)
- **Rust API docs** - Generated from source via `cargo doc`, nested at `/rustdoc` in built site
- **[CLAUDE.md](CLAUDE.md)** - AI assistant routing guide for navigating documentation
- **[README.md](README.md)** (this file) - Quick start guide and basic project structure

The documentation is automatically deployed to **[GitHub Pages](https://emersonmde.github.io/daedalus/)** on every push to `main`.

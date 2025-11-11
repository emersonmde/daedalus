# ADR-002: QEMU 9.0+ Requirement

**Status**: Accepted
**Date**: 2025-11-09
**Decision**: DaedalusOS requires QEMU 9.0 or newer for emulation testing.

## Context

QEMU is the primary tool for kernel development and testing:
- **Fast iteration**: Test changes without SD card flashing
- **Debugging**: GDB integration, semihosting for test output
- **CI/CD**: Automated testing in GitHub Actions

However, QEMU's Raspberry Pi support evolved over time:
- **QEMU 6.1**: Added `raspi3b` (Pi 3) machine type
- **QEMU 6.2**: Improved Pi 3 emulation
- **QEMU 8.x**: Various improvements, but no Pi 4
- **QEMU 9.0** (April 2024): Added `raspi4b` machine type for Pi 4

## Problem

Many Linux distributions ship older QEMU versions:
- **Ubuntu 22.04 LTS**: QEMU 6.2 (no raspi4b)
- **Ubuntu 24.04 LTS**: QEMU 8.2 (still no raspi4b!)
- **Ubuntu 24.10+**: QEMU 9.0+ (has raspi4b)

Installing via `apt install qemu-system-aarch64` on Ubuntu 22.04/24.04 results in:
```
qemu-system-aarch64: unsupported machine type
Use -machine help to list supported machines
```

## Decision

**Require QEMU 9.0 or newer** for DaedalusOS development and testing.

### Implementation

1. **Documentation**: README and setup guides specify QEMU 9.0+ requirement
2. **CI/CD**: GitHub Actions builds QEMU 9.2 from source with caching
3. **Verification**: `qemu-system-aarch64 -M help | grep raspi` must show `raspi4b`

## Rationale

### Why Not Fallback to raspi3b?

Using `raspi3b` machine type (Pi 3 emulation) was considered but rejected:

**Hardware differences**:
- Different MMIO base (`0x3F000000` vs `0xFE000000`)
- Different UART clock (48 MHz vs 54 MHz)
- Different interrupt controller (ARM local vs GIC-400)
- Missing Pi 4-specific features

**Code impact**:
- Would require conditional compilation (#[cfg]) for QEMU vs hardware
- Breaks "one platform" philosophy (see [ADR-001](adr-001-pi-only.md))
- Tests wouldn't validate real hardware behavior

### Why Not Wait for Distribution Packages?

**Timeline reality**:
- Ubuntu 24.04 LTS released April 2024, still ships QEMU 8.2
- Ubuntu 26.04 LTS (April 2026) will likely have QEMU 10+
- Can't wait 1-2 years for package availability

**Alternative**: Build from source or use newer Ubuntu (24.10+).

## Consequences

### Positive

- **Accurate emulation**: Tests run on Pi 4-equivalent environment
- **Single codebase**: No QEMU-specific workarounds
- **Future-proof**: Latest QEMU features available

### Negative

- **Setup complexity**: Users on older Ubuntu must build from source
- **CI build time**: First GH Actions run takes ~4 minutes to compile QEMU
- **Storage**: QEMU build artifacts ~300 MB (mitigated by caching)

## Reversal Plan

This decision will naturally reverse itself as Linux distributions catch up:

**When distribution packages suffice**:
1. Update README to recommend `apt install qemu-system-aarch64` (1 line change)
2. Simplify CI workflow to use apt instead of building from source
3. Remove QEMU build caching steps from GitHub Actions
4. Update ADR-002 status to "Superseded by standard packages"

**Estimated timeline**: Ubuntu 26.04 LTS (April 2026) will likely ship QEMU 10+

**Cost of reversal**: Minimal (simplification, not refactoring)

**Triggers for early reversal**:
- Ubuntu backports QEMU 9.0+ to 24.04 LTS (check `ubuntu-proposed`)
- Raspberry Pi official QEMU binaries become available
- CI environment switches to newer Ubuntu version

This is a **temporary workaround** that will age out naturally.

## Implementation Options

###Option 1: Build QEMU from Source (Recommended)

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

# Add to PATH
export PATH="$HOME/qemu-install/bin:$PATH"
```

**Pros**: Full control, latest version
**Cons**: ~4 minute build time, 300 MB disk space

### Option 2: Upgrade to Ubuntu 24.10+

```bash
# Check current version
lsb_release -a

# Upgrade if on 24.04 or earlier
# (Follow Ubuntu upgrade guide)
```

**Pros**: Simple `apt install`
**Cons**: Major OS upgrade, may break other tools

### Option 3: Use Pre-built Binary

**Status**: Not available. QEMU only distributes source tarballs.

## CI/CD Strategy

GitHub Actions (`.github/workflows/ci.yml`):

```yaml
- name: Cache QEMU build
  uses: actions/cache@v4
  with:
    path: ~/qemu-install
    key: qemu-9.2.0-aarch64

- name: Build QEMU 9.2
  if: cache-miss
  run: |
    # Build from source (first run only)

- name: Run tests
  run: cargo test  # Uses cached QEMU
```

**First run**: ~8 minutes total (4 min QEMU build + 4 min tests)
**Subsequent runs**: ~4 minutes (cached QEMU, tests only)

## Verification

Check QEMU version and raspi4b support:

```bash
$ qemu-system-aarch64 --version
QEMU emulator version 9.2.0

$ qemu-system-aarch64 -M help | grep raspi
raspi0               Raspberry Pi Zero (revision 1.2)
raspi1ap             Raspberry Pi A+ (revision 1.1)
raspi2b              Raspberry Pi 2B (revision 1.1)
raspi3ap             Raspberry Pi 3A+ (revision 1.0)
raspi3b              Raspberry Pi 3B (revision 1.2)
raspi4b              Raspberry Pi 4B (revision 1.2)  ← Must be present
```

## Related Decisions

- [ADR-001: Pi 4 Only](adr-001-pi-only.md) - Why we need raspi4b specifically

## References

- [QEMU 9.0 Release Notes](https://www.qemu.org/2024/04/23/qemu-9-0-0/)
- [QEMU raspi4b Documentation](https://www.qemu.org/docs/master/system/arm/raspi.html)
- [GitHub Actions Workflow](https://github.com/yourusername/daedalus-os/blob/main/.github/workflows/ci.yml)

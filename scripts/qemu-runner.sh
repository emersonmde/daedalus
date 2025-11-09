#!/bin/sh
# Runner script for cargo run and cargo test
# Converts ELF to binary and launches QEMU with semihosting
#
# Environment Variables:
#   QEMU_DETERMINISTIC - Set to "1" to enable deterministic timing mode
#                        Uses -icount for reproducible timing at the cost of slower execution
#                        (disables KVM acceleration, ~10-100x slower)
#                        Useful if timing tests become flaky in CI

set -eu

# Validate input
if [ $# -lt 1 ]; then
    echo "Usage: $0 <elf-file> [test-args...]" >&2
    echo "" >&2
    echo "Options:" >&2
    echo "  QEMU_DETERMINISTIC=1  Enable deterministic timing (slower)" >&2
    exit 1
fi

ELF_FILE="$1"
# Note: Additional args (like test filters) are ignored here but passed to the
# test binary via semihosting by cargo's test framework
DIR=$(dirname "$ELF_FILE")
IMG_FILE="$DIR/kernel8.img"

# Convert ELF to binary using cargo-binutils
rust-objcopy -O binary "$ELF_FILE" "$IMG_FILE"

# Build QEMU arguments
QEMU_ARGS="-M raspi4b -cpu cortex-a72 -serial stdio -display none -semihosting -kernel $IMG_FILE"

# Add deterministic timing mode if requested
# This decouples guest clock from host clock, making timing tests reproducible
# but significantly slower since it disables hardware acceleration
if [ "${QEMU_DETERMINISTIC:-0}" = "1" ]; then
    echo "Running in deterministic mode (icount) - this will be slower" >&2
    QEMU_ARGS="$QEMU_ARGS -icount shift=0"
fi

# Launch QEMU with semihosting for tests
exec qemu-system-aarch64 $QEMU_ARGS

#!/bin/sh
# Runner script for cargo run and cargo test
# Converts ELF to binary and launches QEMU with semihosting

ELF_FILE="$1"
DIR=$(dirname "$ELF_FILE")
IMG_FILE="$DIR/kernel8.img"

# Convert ELF to binary using cargo-binutils
rust-objcopy -O binary "$ELF_FILE" "$IMG_FILE"

# Launch QEMU with semihosting for tests
exec qemu-system-aarch64 -M raspi4b -cpu cortex-a72 -serial stdio -display none -semihosting -kernel "$IMG_FILE"

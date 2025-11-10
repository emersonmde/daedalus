#!/bin/bash
# Test script for eth-diag command in QEMU
# Sends the command and captures output to verify graceful hardware detection

# Build the kernel
echo "Building kernel..."
cargo build --release 2>&1 | tail -5

# Run QEMU with automatic command input
echo ""
echo "Testing eth-diag in QEMU..."
echo ""

# Send eth-diag command after 2 seconds, then exit after 3 more seconds
(sleep 2; echo "eth-diag"; sleep 3; echo "exit") | \
    timeout 10 qemu-system-aarch64 \
    -machine raspi4b \
    -serial stdio \
    -kernel target/aarch64-daedalus/release/daedalus \
    -display none \
    2>&1 | tee /tmp/eth-diag-test.log

# Check the output
echo ""
echo "=== Test Results ==="
if grep -q "\[WARN\].*Hardware not present" /tmp/eth-diag-test.log; then
    echo "✅ PASS: eth-diag correctly detected no hardware in QEMU"
elif grep -q "\[DIAG\].*Ethernet Hardware Diagnostics" /tmp/eth-diag-test.log; then
    echo "✅ PASS: eth-diag command executed"
    if grep -q "\[SKIP\].*no hardware detected" /tmp/eth-diag-test.log; then
        echo "✅ PASS: Gracefully skipped hardware tests"
    fi
else
    echo "❌ FAIL: eth-diag command not found in output"
    exit 1
fi

echo ""
echo "Full diagnostic output:"
grep "\[DIAG\]\|\[WARN\]\|\[SKIP\]\|\[PASS\]\|\[FAIL\]" /tmp/eth-diag-test.log || echo "(no diagnostic output)"

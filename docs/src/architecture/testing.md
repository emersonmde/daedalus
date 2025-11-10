# Testing Framework

DaedalusOS uses a custom test framework for bare-metal testing in QEMU. This document explains how the testing system works and how to write tests.

## Why a Custom Test Framework?

Rust's standard test framework (`#[test]`) requires the `std` library, which is not available in bare-metal environments (`#![no_std]`). DaedalusOS implements a custom framework that:

- Runs tests directly on bare-metal (in QEMU)
- Provides test output via UART serial console
- Exits QEMU with success/failure status codes
- Supports the same test patterns as standard Rust tests

## Architecture

### Test Harness Entry Point

The test harness is defined in `src/lib.rs`:

```rust
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    kernel_init();
    test_main();
    qemu::exit_success();
}
```

When running `cargo test`, this replaces the normal kernel entry point and:
1. Initializes the kernel (UART, MMU, interrupts, etc.)
2. Runs all test functions
3. Exits QEMU with success status if all tests pass

### Test Runner

The `test_main()` function discovers and runs all tests:

```rust
#[cfg(test)]
fn test_main() {
    println!("\nrunning {} tests\n", TEST_CASES.len());

    for test in TEST_CASES {
        test.run();
    }

    println!("\ntest result: ok. {} passed\n", TEST_CASES.len());
}
```

### Test Case Registration

Tests are registered using the `#[test_case]` attribute macro (NOT `#[test]`):

```rust
#[test_case]
fn test_example() {
    assert_eq!(2 + 2, 4);
}
```

The `#[test_case]` attribute:
1. Marks the function as a test
2. Adds it to the `TEST_CASES` static array
3. Wraps it with test runner logic (name printing, panic handling)

**CRITICAL**: Always use `#[test_case]`, never `#[test]`. Using `#[test]` will cause compilation errors because the standard library test crate is not available.

## Writing Tests

### Unit Tests

Place unit tests in a `tests` module within the file being tested:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_mac_address_new() {
        let mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        assert_eq!(mac.0, [0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
    }

    #[test_case]
    fn test_mac_address_broadcast() {
        let mac = MacAddress::broadcast();
        assert!(mac.is_broadcast());
        assert_eq!(mac.0, [0xFF; 6]);
    }
}
```

### Integration Tests

Integration tests are placed in the main test module in `src/lib.rs`:

```rust
#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_kernel_init() {
    kernel_init();  // Should not panic
}
```

### Assertions

All standard Rust assertion macros work:

```rust
assert!(condition);
assert_eq!(left, right);
assert_ne!(left, right);
debug_assert!(condition);  // Only in debug builds
```

When an assertion fails, the test panics and the panic handler prints the error message.

## Test Organization

### Pure Function Tests

Test pure functions (no hardware interaction) extensively:

```rust
#[test_case]
fn test_ethernet_frame_parse() {
    let mut buffer = [0u8; 64];
    buffer[0..6].copy_from_slice(&[0xFF; 6]);  // Dest MAC
    buffer[6..12].copy_from_slice(&[0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);  // Src MAC
    buffer[12..14].copy_from_slice(&[0x08, 0x06]);  // EtherType (ARP)
    buffer[14..20].copy_from_slice(b"Hello!");  // Payload

    let frame = EthernetFrame::parse(&buffer[..20]).unwrap();

    assert_eq!(frame.dest_mac, MacAddress::broadcast());
    assert_eq!(frame.ethertype, ETHERTYPE_ARP);
    assert_eq!(frame.payload, b"Hello!");
}
```

**Why test pure functions?**
- No hardware required (works in QEMU)
- Fast to run (no delays or I/O)
- Deterministic (same input always gives same output)
- High code coverage achievable

### Hardware Tests

Test hardware that QEMU supports:

```rust
#[test_case]
fn test_uart_write_byte() {
    uart::write_byte(b'A');
    uart::write_byte(b'B');
    uart::write_byte(b'C');
    println!();  // Newline
}

#[test_case]
fn test_timer_counter_increments() {
    let before = SystemTimer::counter();
    SystemTimer::delay_us(100);
    let after = SystemTimer::counter();
    assert!(after > before);
}
```

### Skipping Hardware-Only Tests

**Do NOT use `#[ignore]` for hardware-only tests.** If a test can only run on real hardware (not QEMU), don't write it as a test case. Instead:

1. **Test the pure functions** that will be used on hardware
2. **Create diagnostic commands** for manual hardware testing (e.g., `eth-diag`)

**Rationale**: The project is never built or tested on actual hardware via `cargo test`, so ignored tests serve no purpose and create maintenance burden.

Example of the **wrong approach**:
```rust
#[test_case]
#[ignore]  // ❌ Don't do this
fn test_ethernet_tx_on_hardware() {
    // This test will never run in CI or during development
}
```

Example of the **right approach**:
```rust
// src/net/ethernet.rs - Test the pure functions
#[test_case]
fn test_ethernet_frame_write() {
    let frame = EthernetFrame::new(dest, src, ETHERTYPE_IPV4, payload);
    let mut buffer = [0u8; 128];
    let size = frame.write_to(&mut buffer).unwrap();
    // Verify serialization is correct
}

// src/drivers/genet.rs - Provide diagnostic command
pub fn diagnostic(&self) -> bool {
    println!("[DIAG] Checking Ethernet hardware...");
    // Step-by-step hardware validation with verbose output
}
```

## Running Tests

### All Tests
```bash
cargo test
```

This runs all tests in QEMU and shows output like:
```
running 65 tests

test daedalus::net::ethernet::tests::test_mac_address_new ... ok
test daedalus::net::ethernet::tests::test_mac_address_broadcast ... ok
test daedalus::drivers::timer::tests::test_delay_us_actually_delays ... ok
...

test result: ok. 65 passed
```

### Specific Test Module
```bash
cargo test --test <test_name>
```

### Test Output

Tests print to UART, which appears in the console:
- Test names as they run
- Assertion failures with file:line information
- Final pass/fail summary

### QEMU Exit Codes

The test framework uses QEMU semihosting to exit with status codes:
- Exit code 0: All tests passed
- Exit code 1: Test failure or panic
- Exit code 2: QEMU error

See `src/qemu.rs` for implementation details.

## Deterministic Timing Tests

Some timing tests may be flaky in CI environments due to host load. Enable deterministic mode:

```bash
QEMU_DETERMINISTIC=1 cargo test
```

This uses QEMU's `-icount` flag to decouple guest clock from host, making timing perfectly reproducible at the cost of 10-100x slower execution.

Current timing tests use 25% tolerance to handle normal CI variability without needing this flag.

## Test Statistics (Milestone #12)

Current test coverage:

| Category | Tests | Description |
|----------|-------|-------------|
| **Network protocols** | 30 | Ethernet frames, MAC addresses, ARP packets |
| **GENET driver** | 4 | Register offsets, MDIO encoding, PHY constants |
| **Timer** | 6 | Counter, delays, uptime, monotonicity |
| **Allocator** | 6 | Box, Vec, String, capacity, stats |
| **UART** | 6 | Write byte/string, newlines, locking |
| **Shell** | 5 | Command parsing, whitespace handling |
| **Formatting** | 5 | println!, integers, padding, Debug trait |
| **Exception** | 1 | Vector installation |
| **Kernel init** | 2 | Initialization, version output |
| **Total** | **65** | All passing in QEMU |

## Troubleshooting

### Error: "can't find crate for 'test'"

**Problem**: You used `#[test]` instead of `#[test_case]`.

**Solution**: Replace all `#[test]` with `#[test_case]`:
```bash
# In the affected file:
sed -i 's/#\[test\]/#[test_case]/g' src/path/to/file.rs
```

### Error: "no tests to run"

**Problem**: Tests not registered in `TEST_CASES` array.

**Solution**: Ensure you're using `#[test_case]` attribute, not `#[test]` or custom test functions.

### Test Hangs in QEMU

**Problem**: Test enters infinite loop or waits forever.

**Solution**:
1. Use `cargo test` with default timeout (2 minutes)
2. Check for blocking operations (e.g., MDIO reads with no hardware)
3. Add timeout to `cargo test` invocation: `timeout 30 cargo test`

### Timing Test Flakiness

**Problem**: Tests like `test_delay_us_actually_delays` fail intermittently.

**Solution**: Use `QEMU_DETERMINISTIC=1` or increase tolerance in assertions.

## Best Practices

1. **Use `#[test_case]`**, not `#[test]` - This is the most common mistake
2. **Test pure functions extensively** - No hardware = fast, reliable tests
3. **Use diagnostic commands for hardware** - Better than ignored tests
4. **Keep tests fast** - Avoid long delays unless necessary
5. **Test edge cases** - Empty inputs, boundary values, invalid data
6. **Use descriptive test names** - `test_mac_address_broadcast` not `test_mac1`
7. **Group related tests** - One `#[cfg(test)] mod tests` per module
8. **Document non-obvious tests** - Explain what you're testing and why

## Related Documentation

- [Boot Sequence](boot-sequence.md) - How kernel initialization works
- [UART Driver](../hardware/uart-pl011.md) - Test output mechanism
- [QEMU Integration](../decisions/adr-002-qemu-9.md) - QEMU requirements

## External References

- Rust Custom Test Frameworks: <https://os.phil-opp.com/testing/>
- Blog OS Testing Chapter (inspiration for this framework)

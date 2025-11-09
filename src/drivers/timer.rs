//! BCM2711 System Timer driver.
//!
//! The System Timer is a 64-bit free-running counter that increments at 1 MHz,
//! regardless of CPU or core clock speeds. This provides a stable timing source
//! for delays, scheduling, and performance measurement.
//!
//! ## Hardware Characteristics
//!
//! - **Frequency**: 1 MHz (1 microsecond per tick)
//! - **Width**: 64-bit counter (won't overflow for ~584,942 years)
//! - **Cannot be stopped**: The counter always runs
//! - **Cannot be reset**: Counter value cannot be set
//!
//! ## Register Layout
//!
//! | Offset | Register | Purpose |
//! |--------|----------|---------|
//! | +0x00  | CS       | Control/Status (interrupt flags) |
//! | +0x04  | CLO      | Counter Lower 32 bits |
//! | +0x08  | CHI      | Counter Upper 32 bits |
//! | +0x0C  | C0       | Compare 0 (used by GPU) |
//! | +0x10  | C1       | Compare 1 (available for ARM) |
//! | +0x14  | C2       | Compare 2 (used by GPU) |
//! | +0x18  | C3       | Compare 3 (available for ARM) |
//!
//! ## References
//!
//! - BCM2711 ARM Peripherals: Section 10
//! - Base address: 0xFE003000 (ARM physical)
//! - OSDev Wiki: <https://wiki.osdev.org/BCM_System_Timer>

use core::ptr;

/// Base address of the System Timer peripheral (BCM2711 on Pi 4).
///
/// This is the ARM physical address. The bus address in datasheets shows 0x7E003000,
/// but ARM cores must use 0xFE000000 base for peripherals.
const TIMER_BASE: usize = 0xFE003000;

/// System Timer register offsets from TIMER_BASE.
#[allow(dead_code)] // Some registers will be used in Milestone #9 (interrupts)
mod offset {
    pub const CS: usize = 0x00; // Control/Status
    pub const CLO: usize = 0x04; // Counter Lower 32 bits
    pub const CHI: usize = 0x08; // Counter Higher 32 bits
    pub const C0: usize = 0x0C; // Compare 0 (GPU)
    pub const C1: usize = 0x10; // Compare 1 (ARM available)
    pub const C2: usize = 0x14; // Compare 2 (GPU)
    pub const C3: usize = 0x18; // Compare 3 (ARM available)
}

/// System Timer peripheral interface.
///
/// Provides access to the 64-bit free-running counter and delay functions.
pub struct SystemTimer;

impl SystemTimer {
    /// Read the lower 32 bits of the counter.
    ///
    /// The counter increments at 1 MHz (1 tick = 1 microsecond).
    /// To get the full 64-bit value, use [`read_counter()`](Self::read_counter).
    #[inline]
    fn read_clo() -> u32 {
        // SAFETY: Reading from CLO register is safe because:
        // 1. TIMER_BASE + offset::CLO is a valid MMIO address for the System Timer
        // 2. volatile_read prevents compiler optimizations that could cache stale values
        // 3. Reading CLO has no side effects (it's a read-only counter register)
        // 4. The pointer is properly aligned (u32 at 4-byte aligned address)
        // 5. System Timer peripheral is always present on BCM2711
        unsafe { ptr::read_volatile((TIMER_BASE + offset::CLO) as *const u32) }
    }

    /// Read the upper 32 bits of the counter.
    ///
    /// Combined with CLO, this forms a 64-bit counter value.
    #[inline]
    fn read_chi() -> u32 {
        // SAFETY: Reading from CHI register is safe because:
        // 1. TIMER_BASE + offset::CHI is a valid MMIO address for the System Timer
        // 2. volatile_read prevents compiler optimizations that could cache stale values
        // 3. Reading CHI has no side effects (it's a read-only counter register)
        // 4. The pointer is properly aligned (u32 at 4-byte aligned address)
        // 5. System Timer peripheral is always present on BCM2711
        unsafe { ptr::read_volatile((TIMER_BASE + offset::CHI) as *const u32) }
    }

    /// Read the full 64-bit counter value.
    ///
    /// Returns the number of microseconds elapsed since boot (or timer initialization).
    ///
    /// ## Implementation Note
    ///
    /// The 64-bit counter is split across two 32-bit registers (CLO and CHI).
    /// We must handle potential rollover of CLO during the read:
    ///
    /// 1. Read CHI (upper bits)
    /// 2. Read CLO (lower bits)
    /// 3. Read CHI again
    /// 4. If CHI changed, CLO rolled over between reads - use new CHI value
    ///
    /// This ensures we never return an inconsistent value like (old_hi, new_lo).
    pub fn read_counter() -> u64 {
        loop {
            let hi1 = Self::read_chi();
            let lo = Self::read_clo();
            let hi2 = Self::read_chi();

            // If CHI hasn't changed, we have a consistent reading
            if hi1 == hi2 {
                return ((hi1 as u64) << 32) | (lo as u64);
            }
            // Otherwise, CLO rolled over between reads - try again
        }
    }

    /// Get the current timestamp in microseconds.
    ///
    /// This is an alias for [`read_counter()`](Self::read_counter) with a more
    /// descriptive name for timing purposes.
    #[inline]
    pub fn timestamp_us() -> u64 {
        Self::read_counter()
    }

    /// Delay execution for the specified number of microseconds.
    ///
    /// This is a busy-wait delay that polls the timer counter.
    ///
    /// ## Accuracy
    ///
    /// - Resolution: 1 microsecond (timer runs at 1 MHz)
    /// - Overhead: ~2-5 microseconds of function call overhead
    /// - For delays < 10 microseconds, actual delay may be longer
    ///
    /// ## Example
    ///
    /// ```ignore
    /// use daedalus::drivers::timer::SystemTimer;
    ///
    /// // Delay for 1 millisecond
    /// SystemTimer::delay_us(1000);
    /// ```
    pub fn delay_us(microseconds: u64) {
        let start = Self::read_counter();
        let target = start.wrapping_add(microseconds);

        // Handle counter wrap-around (unlikely but possible after ~584,942 years)
        if target < start {
            // Wait for wrap
            while Self::read_counter() >= start {}
        }

        // Wait until we reach the target
        while Self::read_counter() < target {}
    }

    /// Delay execution for the specified number of milliseconds.
    ///
    /// This is a convenience wrapper around [`delay_us()`](Self::delay_us).
    ///
    /// ## Example
    ///
    /// ```ignore
    /// use daedalus::drivers::timer::SystemTimer;
    ///
    /// // Delay for 100 milliseconds
    /// SystemTimer::delay_ms(100);
    /// ```
    #[inline]
    pub fn delay_ms(milliseconds: u64) {
        Self::delay_us(milliseconds.saturating_mul(1000));
    }

    /// Get the current uptime in seconds since boot.
    ///
    /// Returns the integer number of seconds the system has been running.
    /// For sub-second precision, use [`timestamp_us()`](Self::timestamp_us).
    pub fn uptime_seconds() -> u64 {
        Self::read_counter() / 1_000_000
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_timer_counter_increments() {
        let t1 = SystemTimer::read_counter();
        let t2 = SystemTimer::read_counter();

        // Counter should always increase (or stay the same if read very quickly)
        assert!(t2 >= t1, "Timer counter went backwards: {} -> {}", t1, t2);
    }

    #[test_case]
    fn test_timer_timestamp_alias() {
        let c1 = SystemTimer::read_counter();
        let t1 = SystemTimer::timestamp_us();

        // timestamp_us should be very close to read_counter (within a few microseconds)
        let diff = t1.abs_diff(c1);
        assert!(
            diff < 10,
            "timestamp_us differs from read_counter by {} us",
            diff
        );
    }

    #[test_case]
    fn test_delay_us_actually_delays() {
        let start = SystemTimer::read_counter();
        SystemTimer::delay_us(100); // 100 microseconds
        let end = SystemTimer::read_counter();

        let elapsed = end - start;
        // Should be at least 100us, but allow some overhead
        assert!(elapsed >= 100, "delay_us(100) only delayed {} us", elapsed);
        // Shouldn't be wildly longer (allow 50us of overhead)
        assert!(
            elapsed < 150,
            "delay_us(100) took {} us (too long)",
            elapsed
        );
    }

    #[test_case]
    fn test_delay_ms() {
        let start = SystemTimer::read_counter();
        SystemTimer::delay_ms(2); // 2 milliseconds = 2000 microseconds
        let end = SystemTimer::read_counter();

        let elapsed = end - start;
        // Should be at least 2000us
        assert!(elapsed >= 2000, "delay_ms(2) only delayed {} us", elapsed);
        // Shouldn't be wildly longer (allow 100us of overhead)
        assert!(elapsed < 2100, "delay_ms(2) took {} us (too long)", elapsed);
    }

    #[test_case]
    fn test_uptime_seconds() {
        let uptime = SystemTimer::uptime_seconds();
        // Should be a reasonable value (system hasn't been running for years)
        // In QEMU tests, uptime will be very small
        assert!(
            uptime < 3600,
            "Uptime is {} seconds (> 1 hour), seems wrong",
            uptime
        );
    }

    #[test_case]
    fn test_counter_is_monotonic() {
        // Take multiple readings and ensure they're always increasing
        let mut last = SystemTimer::read_counter();

        for _ in 0..10 {
            let current = SystemTimer::read_counter();
            assert!(
                current >= last,
                "Timer is not monotonic: {} -> {}",
                last,
                current
            );
            last = current;
        }
    }
}

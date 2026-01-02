//! Mutex implementation for DaedalusOS
//!
//! This module provides `Mutex`, a spinlock that disables interrupts while
//! the lock is held. In kernel code, this is what a mutex should do—handle
//! interrupt safety automatically.
//!
//! ## Why disable interrupts?
//!
//! Regular spinlocks deadlock if an interrupt tries to acquire a lock
//! that the interrupted code already holds:
//!
//! ```text
//! 1. Thread acquires spinlock
//! 2. Interrupt fires (e.g., RX packet arrives)
//! 3. Interrupt handler tries to acquire same spinlock
//! 4. DEADLOCK: Handler spins forever, thread never resumes
//! ```
//!
//! Disabling interrupts prevents this by ensuring no interrupt can fire
//! while we hold the lock.
//!
//! ## Implementation
//!
//! This is the standard kernel pattern (Linux uses `spin_lock_irqsave()`):
//! 1. Save current IRQ state
//! 2. Disable interrupts
//! 3. Acquire spinlock
//! 4. On drop: Release lock, restore IRQ state

use core::arch::asm;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// Mutex that disables interrupts while locked
///
/// This is a spinlock that automatically disables interrupts to prevent
/// deadlocks between normal code and interrupt handlers.
///
/// # Example
/// ```ignore
/// static GENET: Mutex<GenetController> = Mutex::new(GenetController::new());
///
/// let mut genet = GENET.lock();
/// genet.transmit(frame)?;
/// // Interrupts re-enabled when guard is dropped
/// ```
pub struct Mutex<T> {
    inner: UnsafeCell<T>,
    locked: AtomicBool,
}

// SAFETY: Mutex is Sync because:
// - Access to inner data is protected by atomic `locked` flag
// - IRQs are disabled while lock is held (prevents concurrent interrupt access)
// - Only one thread/CPU can hold the lock at a time
unsafe impl<T: Send> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    /// Create a new mutex
    pub const fn new(value: T) -> Self {
        Self {
            inner: UnsafeCell::new(value),
            locked: AtomicBool::new(false),
        }
    }

    /// Acquire the lock (disables interrupts)
    ///
    /// Returns a guard that will restore interrupts when dropped.
    /// Spins if the lock is already held (should be rare - locks held briefly).
    pub fn lock(&self) -> MutexGuard<'_, T> {
        // Save current IRQ state and disable IRQs
        let irq_state = IrqState::disable();

        // Acquire spinlock (safe now - interrupts can't fire)
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Spin-wait (lock is held by another CPU or nested lock attempt)
            core::hint::spin_loop();
        }

        MutexGuard {
            mutex: self,
            irq_state,
        }
    }
}

/// Guard returned by `Mutex::lock()`
///
/// Restores interrupt state when dropped.
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
    irq_state: IrqState,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: Lock is held, so we have exclusive access
        unsafe { &*self.mutex.inner.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: Lock is held, so we have exclusive access
        unsafe { &mut *self.mutex.inner.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // Release spinlock
        self.mutex.locked.store(false, Ordering::Release);

        // Restore IRQ state
        self.irq_state.restore();
    }
}

/// Saved IRQ state (from DAIF register)
///
/// DAIF = Debug, SError, IRQ, FIQ mask bits
/// We only care about IRQ (bit 1) for this implementation.
struct IrqState {
    daif: u64,
}

impl IrqState {
    /// Disable IRQs and return previous state
    ///
    /// Reads DAIF register, sets IRQ mask bit (bit 1), writes back.
    fn disable() -> Self {
        let daif: u64;

        // SAFETY: Reading/writing DAIF is safe because:
        // 1. DAIF is accessible at EL1 (our exception level)
        // 2. MSR daif_set disables interrupts atomically
        // 3. We save the old value to restore later
        unsafe {
            // Read current DAIF value
            asm!("mrs {}, daif", out(reg) daif, options(nomem, nostack));

            // Disable IRQs (set bit 1 in DAIF)
            // Using msr daifset is more efficient than read-modify-write
            asm!("msr daifset, #2", options(nomem, nostack));
        }

        Self { daif }
    }

    /// Restore previous IRQ state
    ///
    /// Writes saved DAIF value back to register.
    fn restore(&self) {
        // SAFETY: Restoring DAIF is safe because:
        // 1. We're restoring a value we previously saved
        // 2. DAIF write is atomic (single MSR instruction)
        unsafe {
            asm!("msr daif, {}", in(reg) self.daif, options(nomem, nostack));
        }
    }
}

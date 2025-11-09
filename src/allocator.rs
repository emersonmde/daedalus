//! Simple bump allocator for heap memory management.
//!
//! This module provides a basic bump allocator that moves a pointer forward with
//! each allocation. Individual deallocations are no-ops, making this allocator
//! suitable for workloads where memory is allocated but rarely freed, or where
//! memory can be reclaimed in bulk.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;
use spin::Mutex;

/// Bump allocator - simplest allocator, just moves pointer forward
pub struct BumpAllocator {
    heap_start: UnsafeCell<usize>,
    heap_end: UnsafeCell<usize>,
    next: Mutex<usize>,
}

// SAFETY: BumpAllocator is Sync because:
// 1. UnsafeCell<usize> does not implement Sync by default (allows interior mutability)
// 2. heap_start and heap_end are only mutated during init() (unsafe fn with caller requirements)
// 3. All reads of heap_start/heap_end occur after initialization in practice
// 4. next is protected by a Mutex, providing thread-safe access
// 5. The type invariant is that init() is called exactly once before first use (enforced by caller of unsafe init())
unsafe impl Sync for BumpAllocator {}

impl Default for BumpAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl BumpAllocator {
    /// Create a new bump allocator (must call init before use)
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: UnsafeCell::new(0),
            heap_end: UnsafeCell::new(0),
            next: Mutex::new(0),
        }
    }

    /// Initialize the allocator with heap bounds from linker script
    ///
    /// # Safety
    /// The caller must ensure:
    /// - This function is called exactly once before any allocations
    /// - heap_start < heap_end
    /// - The memory range [heap_start, heap_end) is valid, properly aligned, and reserved for heap use
    /// - No concurrent access to this allocator occurs during or after initialization
    pub unsafe fn init(&self, heap_start: usize, heap_end: usize) {
        // Debug assertions to catch configuration errors early (zero cost in release builds)
        debug_assert!(
            heap_start < heap_end,
            "Invalid heap bounds: start=0x{:x} >= end=0x{:x}",
            heap_start,
            heap_end
        );
        debug_assert!(
            heap_start != 0 && heap_end != 0,
            "Heap bounds cannot be zero (start=0x{:x}, end=0x{:x})",
            heap_start,
            heap_end
        );
        debug_assert!(
            (heap_end - heap_start) >= 1024,
            "Heap too small: {} bytes (minimum 1 KB)",
            heap_end - heap_start
        );
        debug_assert_eq!(
            heap_start % 16,
            0,
            "Heap start not 16-byte aligned: 0x{:x}",
            heap_start
        );

        // SAFETY: Writing to UnsafeCells is safe because:
        // 1. The caller guarantees this is called exactly once (per # Safety contract above)
        // 2. The caller guarantees no concurrent access during initialization
        // 3. UnsafeCell::get() returns a mutable pointer, writing through it is safe given exclusivity
        // 4. heap_start and heap_end are usize values, always valid to write
        // 5. The caller guarantees heap_start < heap_end and the range is valid memory
        unsafe {
            *self.heap_start.get() = heap_start;
            *self.heap_end.get() = heap_end;
            *self.next.lock() = heap_start;
        }
    }

    /// Get total heap size in bytes
    pub fn heap_size(&self) -> usize {
        // SAFETY: Reading through UnsafeCell is safe because:
        // 1. UnsafeCells are initialized to 0 in new(), so they always contain valid usize values
        // 2. After init() is called, they contain the heap bounds
        // 3. No concurrent writes occur (init() is unsafe and requires caller to ensure single initialization)
        // 4. Reading a usize through UnsafeCell::get() is always safe (no alignment issues)
        // 5. If init() wasn't called, both are 0, so result is 0 (safe but incorrect semantically)
        unsafe { *self.heap_end.get() - *self.heap_start.get() }
    }

    /// Get used heap size in bytes
    pub fn used(&self) -> usize {
        // SAFETY: Reading through UnsafeCell is safe because:
        // 1. heap_start is initialized to 0 in new(), so it always contains a valid usize value
        // 2. After init() is called, it contains the heap start address
        // 3. No concurrent writes to heap_start occur after init()
        // 4. Reading a usize through UnsafeCell::get() is always safe (no alignment issues)
        // 5. If init() wasn't called, subtraction is still safe (0 - 0 = 0)
        unsafe { *self.next.lock() - *self.heap_start.get() }
    }

    /// Get free heap size in bytes
    pub fn free(&self) -> usize {
        // SAFETY: Reading through UnsafeCell is safe because:
        // 1. heap_end is initialized to 0 in new(), so it always contains a valid usize value
        // 2. After init() is called, it contains the heap end address
        // 3. No concurrent writes to heap_end occur after init()
        // 4. Reading a usize through UnsafeCell::get() is always safe (no alignment issues)
        // 5. If init() wasn't called, result is 0 - 0 = 0 (safe but semantically incorrect)
        unsafe { *self.heap_end.get() - *self.next.lock() }
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: This unsafe block is safe because:
        // 1. The Mutex ensures exclusive access to `next`, preventing concurrent allocations from overlapping
        // 2. Reading heap_end through UnsafeCell is safe (initialized to 0 in new(), valid usize)
        // 3. We check alloc_end > heap_end BEFORE returning a pointer (pre-condition verified)
        // 4. If heap_end is 0 (uninitialized), the check at line 109 causes us to return null (safe OOM)
        // 5. The returned pointer is properly aligned: (next + align - 1) & !(align - 1) ensures alignment
        // 6. The memory range [alloc_start, alloc_end) is within heap bounds (verified by check at line 109)
        // 7. Alignment calculation cannot overflow because layout.align() is power of 2 and <= isize::MAX
        unsafe {
            let mut next = self.next.lock();

            // Align the next pointer to the required alignment
            let alloc_start = (*next + layout.align() - 1) & !(layout.align() - 1);
            let alloc_end = alloc_start + layout.size();

            // Check if we have enough space (also handles uninitialized case where heap_end is 0)
            if alloc_end > *self.heap_end.get() {
                // Out of memory
                return ptr::null_mut();
            }

            // Update next pointer
            *next = alloc_end;

            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator never deallocates individual allocations
        // Memory is only reclaimed when the entire allocator is reset
        // This is safe because:
        // 1. Rust's type system ensures dealloc is only called for previously allocated pointers
        // 2. Not freeing memory doesn't violate memory safety (just wastes space)
        // 3. This is a deliberate design choice for simplicity
    }
}

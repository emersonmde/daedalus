//! Global Packet Pool with Reference Counting
//!
//! This module provides a lock-free packet buffer pool used for zero-copy RX packet handling.
//! The pool allows the interrupt handler to immediately free GENET descriptors while keeping
//! packet data available for sockets to process asynchronously.
//!
//! ## Architecture
//!
//! - **256 buffers**: Matches GENET RX ring descriptor count
//! - **Reference counting**: Multiple sockets can reference same packet (e.g., broadcast)
//! - **Lock-free allocation**: AtomicBitmap for concurrent alloc/free
//! - **Zero-copy**: Sockets hold references to pool buffers, not copies

use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// Maximum number of packet buffers in the pool (matches GENET ring size)
pub const PACKET_POOL_SIZE: usize = 256;

/// Maximum Ethernet frame size (1514 bytes + padding to 2KB for DMA alignment)
pub const MAX_PACKET_SIZE: usize = 2048;

/// Single packet buffer with reference counting
pub struct PacketBuffer {
    /// Pointer to packet data (points into GENET RX buffer, static lifetime)
    data: *const u8,

    /// Length of valid packet data
    len: usize,

    /// Reference count (0 = free, >0 = in use)
    refcount: AtomicU32,

    /// Timestamp when packet was received (from system timer)
    timestamp: u64,
}

// SAFETY: PacketBuffer can be shared between threads because:
// - data is immutable after allocation (read-only access)
// - len is immutable after allocation
// - refcount uses atomic operations
// - timestamp is immutable after allocation
unsafe impl Sync for PacketBuffer {}

impl PacketBuffer {
    /// Create a new empty packet buffer
    const fn new() -> Self {
        Self {
            data: core::ptr::null(),
            len: 0,
            refcount: AtomicU32::new(0),
            timestamp: 0,
        }
    }

    /// Initialize buffer with packet data
    ///
    /// # Arguments
    /// * `data` - Pointer to packet data (must remain valid for buffer lifetime)
    /// * `len` - Length of packet data
    ///
    /// # Safety
    /// Caller must ensure:
    /// - `data` points to valid memory of at least `len` bytes
    /// - `data` has static lifetime (e.g., points into GENET DMA buffer)
    /// - `len` <= MAX_PACKET_SIZE
    unsafe fn init(&mut self, data: *const u8, len: usize) {
        self.data = data;
        self.len = len;
        self.refcount.store(1, Ordering::Release);
        self.timestamp = crate::drivers::timer::SystemTimer::timestamp_us();
    }

    /// Increment reference count
    ///
    /// Returns the previous refcount value.
    fn clone_ref(&self) -> u32 {
        self.refcount.fetch_add(1, Ordering::AcqRel)
    }

    /// Decrement reference count
    ///
    /// Returns true if this was the last reference (refcount reached 0).
    fn drop_ref(&self) -> bool {
        let prev = self.refcount.fetch_sub(1, Ordering::AcqRel);
        prev == 1 // Was last reference
    }

    /// Get packet data as slice
    ///
    /// # Safety
    /// Caller must ensure refcount > 0 (buffer is allocated).
    unsafe fn as_slice(&self) -> &[u8] {
        // SAFETY: Caller guarantees refcount > 0, meaning buffer is allocated and
        // self.data points to valid memory of at least self.len bytes
        unsafe { core::slice::from_raw_parts(self.data, self.len) }
    }

    /// Get receive timestamp
    fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

/// Atomic bitmap for lock-free bit allocation
///
/// Uses 8 × u32 words to track 256 bits (one per packet buffer).
/// Each bit: 0 = free, 1 = allocated
struct AtomicBitmap {
    words: [AtomicU32; 8], // 8 words × 32 bits = 256 bits
}

impl AtomicBitmap {
    /// Create a new bitmap with all bits set to 0 (all free)
    const fn new() -> Self {
        const ATOMIC_ZERO: AtomicU32 = AtomicU32::new(0);
        Self {
            words: [ATOMIC_ZERO; 8],
        }
    }

    /// Acquire a free bit (set to 1)
    ///
    /// Returns the bit index if successful, None if all bits are allocated.
    fn acquire_bit(&self) -> Option<usize> {
        for word_idx in 0..8 {
            let word = &self.words[word_idx];

            // Try to find a free bit in this word
            loop {
                let current = word.load(Ordering::Acquire);

                // Find first zero bit (free buffer)
                let free_bit = (!current).trailing_zeros();
                if free_bit >= 32 {
                    break; // No free bits in this word
                }

                // Try to set the bit
                let mask = 1u32 << free_bit;
                let new_value = current | mask;

                match word.compare_exchange_weak(
                    current,
                    new_value,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Successfully acquired bit
                        let bit_idx = word_idx * 32 + free_bit as usize;
                        return Some(bit_idx);
                    }
                    Err(_) => {
                        // Another thread modified the word, retry
                        continue;
                    }
                }
            }
        }

        None // All bits are allocated
    }

    /// Release a bit (set to 0)
    ///
    /// # Arguments
    /// * `bit_idx` - Bit index to release (0..256)
    ///
    /// # Panics
    /// Panics if bit_idx >= 256 or if the bit was already free (double-free).
    fn release_bit(&self, bit_idx: usize) {
        assert!(bit_idx < 256, "Invalid bit index: {}", bit_idx);

        let word_idx = bit_idx / 32;
        let bit_pos = (bit_idx % 32) as u32;
        let mask = 1u32 << bit_pos;

        let word = &self.words[word_idx];
        let prev = word.fetch_and(!mask, Ordering::AcqRel);

        // Check for double-free
        assert!(prev & mask != 0, "Double-free of packet buffer {}", bit_idx);
    }

    /// Check if a bit is allocated
    #[allow(dead_code)] // Used for diagnostics
    fn is_allocated(&self, bit_idx: usize) -> bool {
        assert!(bit_idx < 256);
        let word_idx = bit_idx / 32;
        let bit_pos = (bit_idx % 32) as u32;
        let mask = 1u32 << bit_pos;

        let word = self.words[word_idx].load(Ordering::Acquire);
        (word & mask) != 0
    }
}

/// Global packet pool
pub struct PacketPool {
    /// Array of 256 packet buffers
    buffers: [PacketBuffer; PACKET_POOL_SIZE],

    /// Bitmap tracking which buffers are allocated
    free_list: AtomicBitmap,

    /// Statistics
    alloc_count: AtomicUsize,
    free_count: AtomicUsize,
    alloc_failures: AtomicUsize,
}

// SAFETY: PacketPool can be shared between threads because:
// - buffers array is protected by atomic operations
// - free_list uses atomic operations
// - statistics use atomic counters
unsafe impl Sync for PacketPool {}

impl PacketPool {
    /// Create a new empty packet pool
    pub const fn new() -> Self {
        const EMPTY_BUFFER: PacketBuffer = PacketBuffer::new();
        Self {
            buffers: [EMPTY_BUFFER; PACKET_POOL_SIZE],
            free_list: AtomicBitmap::new(),
            alloc_count: AtomicUsize::new(0),
            free_count: AtomicUsize::new(0),
            alloc_failures: AtomicUsize::new(0),
        }
    }

    /// Allocate a packet buffer from the pool
    ///
    /// # Arguments
    /// * `data` - Slice containing packet data
    ///
    /// # Returns
    /// Buffer ID (0..256) on success, None if pool is full.
    ///
    /// # Safety
    /// Caller must ensure `data` has static lifetime (e.g., points into GENET DMA buffer).
    /// The data will be referenced (not copied) until the buffer is freed.
    pub unsafe fn alloc(&self, data: &'static [u8]) -> Option<usize> {
        if data.len() > MAX_PACKET_SIZE {
            return None;
        }

        // Find free buffer
        let buffer_id = match self.free_list.acquire_bit() {
            Some(id) => id,
            None => {
                self.alloc_failures.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };

        // Initialize buffer
        // SAFETY: We just acquired this buffer from the free list, so it's safe to initialize.
        // The buffer is exclusively ours (no other thread can access it).
        // data has static lifetime as guaranteed by caller.
        // We cast to *mut because we have exclusive access (just acquired from free list).
        unsafe {
            let buffer_ptr = &self.buffers[buffer_id] as *const PacketBuffer as *mut PacketBuffer;
            (*buffer_ptr).init(data.as_ptr(), data.len());
        }

        self.alloc_count.fetch_add(1, Ordering::Relaxed);
        Some(buffer_id)
    }

    /// Increment reference count for a buffer
    ///
    /// Used when multiple sockets need to reference the same packet (e.g., broadcast).
    ///
    /// # Arguments
    /// * `buffer_id` - Buffer ID (0..256)
    ///
    /// # Panics
    /// Panics if buffer_id >= 256 or buffer refcount is 0 (not allocated).
    pub fn clone_ref(&self, buffer_id: usize) {
        assert!(
            buffer_id < PACKET_POOL_SIZE,
            "Invalid buffer ID: {}",
            buffer_id
        );

        let buffer = &self.buffers[buffer_id];
        let prev_refcount = buffer.clone_ref();

        assert!(
            prev_refcount > 0,
            "Attempted to clone free buffer {}",
            buffer_id
        );
    }

    /// Decrement reference count and free buffer if count reaches 0
    ///
    /// # Arguments
    /// * `buffer_id` - Buffer ID (0..256)
    ///
    /// # Panics
    /// Panics if buffer_id >= 256 or buffer was already free.
    ///
    /// # Note on RX Descriptor Management
    /// This function does NOT call `genet.free_rx_buffer()` to avoid recursive locking.
    /// The interrupt handler is solely responsible for managing RX descriptors:
    /// it calls `free_rx_buffer()` immediately after routing each packet.
    ///
    /// Packet pool buffers and GENET RX descriptors have different lifecycles:
    /// - **RX descriptors**: Interrupt handler receives → routes → frees descriptor (immediate)
    /// - **Pool buffers**: Interrupt allocates → socket holds reference → app reads → frees buffer (delayed)
    pub fn free(&self, buffer_id: usize) {
        assert!(
            buffer_id < PACKET_POOL_SIZE,
            "Invalid buffer ID: {}",
            buffer_id
        );

        let buffer = &self.buffers[buffer_id];
        let was_last_ref = buffer.drop_ref();

        if was_last_ref {
            // Last reference - return buffer to pool
            self.free_list.release_bit(buffer_id);
            self.free_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get packet data as slice
    ///
    /// # Arguments
    /// * `buffer_id` - Buffer ID (0..256)
    ///
    /// # Returns
    /// Slice containing packet data.
    ///
    /// # Panics
    /// Panics if buffer_id >= 256 or buffer is not allocated.
    pub fn get(&self, buffer_id: usize) -> &[u8] {
        assert!(
            buffer_id < PACKET_POOL_SIZE,
            "Invalid buffer ID: {}",
            buffer_id
        );

        let buffer = &self.buffers[buffer_id];

        // SAFETY: We check that refcount > 0 to ensure buffer is allocated
        let refcount = buffer.refcount.load(Ordering::Acquire);
        assert!(
            refcount > 0,
            "Attempted to access free buffer {}",
            buffer_id
        );

        unsafe { buffer.as_slice() }
    }

    /// Get packet timestamp
    ///
    /// # Arguments
    /// * `buffer_id` - Buffer ID (0..256)
    ///
    /// # Returns
    /// Receive timestamp in microseconds.
    pub fn timestamp(&self, buffer_id: usize) -> u64 {
        assert!(buffer_id < PACKET_POOL_SIZE);
        self.buffers[buffer_id].timestamp()
    }

    /// Get pool statistics
    pub fn stats(&self) -> PacketPoolStats {
        PacketPoolStats {
            alloc_count: self.alloc_count.load(Ordering::Relaxed),
            free_count: self.free_count.load(Ordering::Relaxed),
            alloc_failures: self.alloc_failures.load(Ordering::Relaxed),
        }
    }
}

/// Packet pool statistics
#[derive(Debug, Clone, Copy)]
pub struct PacketPoolStats {
    /// Total allocations
    pub alloc_count: usize,

    /// Total frees
    pub free_count: usize,

    /// Allocation failures (pool exhausted)
    pub alloc_failures: usize,
}

/// Global packet pool instance
pub static PACKET_POOL: PacketPool = PacketPool::new();

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;

    #[test_case]
    fn test_atomic_bitmap_acquire_release() {
        let bitmap = AtomicBitmap::new();

        // Acquire first bit
        let bit0 = bitmap.acquire_bit().unwrap();
        assert_eq!(bit0, 0);
        assert!(bitmap.is_allocated(0));

        // Acquire second bit
        let bit1 = bitmap.acquire_bit().unwrap();
        assert_eq!(bit1, 1);
        assert!(bitmap.is_allocated(1));

        // Release first bit
        bitmap.release_bit(bit0);
        assert!(!bitmap.is_allocated(0));

        // Re-acquire should get bit 0 again
        let bit0_again = bitmap.acquire_bit().unwrap();
        assert_eq!(bit0_again, 0);
    }

    #[test_case]
    fn test_atomic_bitmap_exhaustion() {
        let bitmap = AtomicBitmap::new();

        // Acquire all 256 bits
        let mut bits = alloc::vec::Vec::new();
        for _ in 0..256 {
            let bit = bitmap.acquire_bit().unwrap();
            bits.push(bit);
        }

        // Next acquisition should fail
        assert!(bitmap.acquire_bit().is_none());

        // Release one bit
        bitmap.release_bit(bits[100]);

        // Should be able to acquire again
        let bit = bitmap.acquire_bit().unwrap();
        assert_eq!(bit, 100);
    }

    #[test_case]
    fn test_packet_buffer_refcount() {
        let mut buffer = PacketBuffer::new();

        // Initialize buffer
        let data: &[u8] = &[1, 2, 3, 4, 5];
        unsafe {
            // Cast to static lifetime for test
            let static_data: &'static [u8] = core::mem::transmute(data);
            buffer.init(static_data.as_ptr(), static_data.len());
        }

        // Refcount should be 1
        assert_eq!(buffer.refcount.load(Ordering::Acquire), 1);

        // Clone ref (now 2)
        buffer.clone_ref();
        assert_eq!(buffer.refcount.load(Ordering::Acquire), 2);

        // Drop ref (now 1)
        assert!(!buffer.drop_ref());
        assert_eq!(buffer.refcount.load(Ordering::Acquire), 1);

        // Drop ref (now 0)
        assert!(buffer.drop_ref());
        assert_eq!(buffer.refcount.load(Ordering::Acquire), 0);
    }
}

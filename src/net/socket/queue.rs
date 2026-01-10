//! Lock-Free Socket Buffer Queue for TX/RX
//!
//! This module provides Single-Producer Single-Consumer (SPSC) ring buffers
//! for packet transmission and reception. Queues hold Arc<SkBuff> references,
//! enabling zero-overhead packet sharing and automatic memory management.
//!
//! ## Design Pattern
//!
//! Follows the same pattern as the UART RX ring buffer (src/drivers/tty/serial/amba_pl011.rs):
//! - Power-of-2 capacity for efficient modulo via masking
//! - Atomic head/tail indices with Acquire/Release ordering
//! - Single producer, single consumer per queue
//! - No locks required - safe for interrupt context
//!
//! ## Linux Comparison
//!
//! This is equivalent to Linux's socket write queue (TX) and receive queue (RX).
//! See "The Path of a Packet Through the Linux Kernel" Figures 3 & 4.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::net::skbuff::SkBuff;

/// Lock-free SPSC ring buffer for socket buffers
///
/// Used for both TX (egress) and RX (ingress) packet queues.
///
/// # Capacity
///
/// Fixed at 32 sk_buffs per queue (compile-time constant).
/// This handles bursts of ~1ms at typical packet rates (30k pps).
///
/// # Thread Safety
///
/// - **RX Queue**: Producer = interrupt handler, Consumer = socket recv()
/// - **TX Queue**: Producer = socket send(), Consumer = protocol handler
/// - **Ordering**: Acquire/Release prevents reordering across boundaries
///
/// # Memory Ordering
///
/// Producer:
/// 1. Write sk_buff to queue[head]
/// 2. Store head with Release (ensures write visible before advancing head)
///
/// Consumer:
/// 1. Load head with Acquire (ensures we see producer's writes)
/// 2. Read sk_buff from queue[tail]
/// 3. Store tail with Release
pub struct SkBuffQueue {
    /// Ring buffer of sk_buff references
    /// Note: One slot always remains empty to distinguish full from empty
    queue: [Option<Arc<SkBuff>>; Self::CAPACITY],

    /// Write index (producer only)
    head: AtomicUsize,

    /// Read index (consumer only)
    tail: AtomicUsize,
}

impl SkBuffQueue {
    /// Queue capacity (MUST be power of 2 for efficient masking)
    /// Note: Actual usable capacity is CAPACITY - 1 (one slot reserved to distinguish full from empty)
    ///
    /// Set to 8192 packets (~12 MB at 1500 bytes/packet worst case, <20% of 64 MB heap).
    /// Linux can buffer 1-6 MB (often more for TCP); this exceeds Linux's typical capacity.
    pub const CAPACITY: usize = 8192;

    /// Bit mask for wrapping indices (CAPACITY - 1)
    const MASK: usize = Self::CAPACITY - 1;

    /// Create a new empty queue
    pub const fn new() -> Self {
        const NONE: Option<Arc<SkBuff>> = None;
        Self {
            queue: [NONE; Self::CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Enqueue an sk_buff (producer)
    ///
    /// # Arguments
    /// * `skb` - sk_buff reference to enqueue (Arc cloned into queue)
    ///
    /// # Returns
    /// * `Ok(())` if sk_buff was enqueued
    /// * `Err(skb)` if queue is full (returns sk_buff back to caller)
    ///
    /// # Safety
    /// Must only be called from single producer.
    ///
    /// # Example
    /// ```ignore
    /// // In interrupt handler (RX queue)
    /// if socket.rx_queue.enqueue(skb.clone()).is_err() {
    ///     // Queue full - drop packet
    ///     drop(skb);
    /// }
    /// ```
    pub fn enqueue(&self, skb: Arc<SkBuff>) -> Result<(), Arc<SkBuff>> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        // Check if queue is full
        // Queue is full when (head + 1) % CAPACITY == tail
        let next_head = (head + 1) & Self::MASK;
        if next_head == tail {
            return Err(skb); // Queue full
        }

        // Write sk_buff to queue
        // SAFETY: head < CAPACITY due to masking, and we're the only writer
        unsafe {
            let queue_ptr = self.queue.as_ptr() as *mut Option<Arc<SkBuff>>;
            queue_ptr.add(head).write(Some(skb));
        }

        // Advance head with Release ordering (makes write visible)
        self.head.store(next_head, Ordering::Release);

        Ok(())
    }

    /// Dequeue an sk_buff (consumer)
    ///
    /// # Returns
    /// * `Some(skb)` if sk_buff was available
    /// * `None` if queue is empty
    ///
    /// # Safety
    /// Must only be called from single consumer.
    ///
    /// # Example
    /// ```ignore
    /// // In socket recv()
    /// if let Some(skb) = socket.rx_queue.dequeue() {
    ///     // Process packet
    ///     let data = skb.data().to_vec();
    ///     // skb dropped here, refcount decremented
    /// }
    /// ```
    pub fn dequeue(&self) -> Option<Arc<SkBuff>> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        // Check if queue is empty
        if head == tail {
            return None;
        }

        // Read sk_buff from queue
        // SAFETY: tail < CAPACITY due to masking, and we're the only reader
        let skb = unsafe {
            let queue_ptr = self.queue.as_ptr();
            queue_ptr.add(tail).read()
        };

        // Advance tail with Release ordering
        let next_tail = (tail + 1) & Self::MASK;
        self.tail.store(next_tail, Ordering::Release);

        skb
    }

    /// Drain all sk_buffs from queue (used during socket close)
    ///
    /// Returns an iterator that dequeues and drops all sk_buffs.
    /// This prevents sk_buff leaks when a socket is closed with pending packets.
    ///
    /// # Safety
    /// Must only be called when no producers/consumers are active (socket closing).
    ///
    /// # Example
    /// ```ignore
    /// // In socket close path
    /// for skb in socket.rx_queue.drain() {
    ///     // skb dropped automatically (Arc refcount decremented)
    /// }
    /// ```
    pub fn drain(&self) -> DrainIterator<'_> {
        DrainIterator { queue: self }
    }

    /// Get current queue depth (approximate, may be stale)
    ///
    /// This is safe to call from any context but the result may be outdated
    /// by the time it's used (due to concurrent enqueue/dequeue).
    ///
    /// # Returns
    /// Number of sk_buffs currently in queue (0..CAPACITY)
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);

        // Calculate depth accounting for wraparound
        (head.wrapping_sub(tail)) & Self::MASK
    }

    /// Check if queue is empty (approximate)
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        head == tail
    }

    /// Check if queue is full (approximate)
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        let next_head = (head + 1) & Self::MASK;
        next_head == tail
    }
}

/// Iterator for draining sk_buffs from queue
///
/// This iterator repeatedly dequeues sk_buffs until the queue is empty.
/// Each sk_buff is dropped automatically when the iterator advances.
pub struct DrainIterator<'a> {
    queue: &'a SkBuffQueue,
}

impl<'a> Iterator for DrainIterator<'a> {
    type Item = Arc<SkBuff>;

    fn next(&mut self) -> Option<Self::Item> {
        self.queue.dequeue()
    }
}

// SAFETY: SkBuffQueue can be shared between threads because:
// - Single producer modifies head, single consumer modifies tail (no data races)
// - Acquire/Release ordering prevents reordering across synchronization points
// - Queue indices are always in bounds due to power-of-2 masking
// - Arc<SkBuff> is Send + Sync (safe to share between threads)
unsafe impl Sync for SkBuffQueue {}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test sk_buff
    fn test_skb(id: usize) -> Arc<SkBuff> {
        let data = alloc::vec![id as u8; 64];
        SkBuff::from_dma(&data).unwrap()
    }

    #[test_case]
    fn test_queue_new_empty() {
        let queue = SkBuffQueue::new();
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
        assert!(!queue.is_full());
    }

    #[test_case]
    fn test_queue_enqueue_dequeue() {
        let queue = SkBuffQueue::new();

        let skb1 = test_skb(42);

        // Enqueue
        assert!(queue.enqueue(skb1.clone()).is_ok());
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());

        // Dequeue
        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.len(), 64);
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test_case]
    fn test_queue_full() {
        let queue = SkBuffQueue::new();

        // Fill queue (CAPACITY - 1 because one slot stays empty)
        for i in 0..(SkBuffQueue::CAPACITY - 1) {
            let skb = test_skb(i);
            assert!(queue.enqueue(skb).is_ok());
        }

        assert!(queue.is_full());

        // Next enqueue should fail
        let skb = test_skb(999);
        assert!(queue.enqueue(skb).is_err());
    }

    #[test_case]
    fn test_queue_wraparound() {
        let queue = SkBuffQueue::new();

        // Fill and drain multiple times
        for round in 0..3 {
            // Fill queue
            for i in 0..(SkBuffQueue::CAPACITY - 1) {
                let skb = test_skb(round * 100 + i);
                assert!(queue.enqueue(skb).is_ok());
            }

            // Drain queue
            for _i in 0..(SkBuffQueue::CAPACITY - 1) {
                assert!(queue.dequeue().is_some());
            }

            assert!(queue.is_empty());
        }
    }

    #[test_case]
    fn test_queue_fifo_order() {
        let queue = SkBuffQueue::new();

        // Enqueue sk_buffs
        for i in 0..10 {
            let skb = test_skb(i);
            queue.enqueue(skb).unwrap();
        }

        // Dequeue should return in FIFO order
        for i in 0..10 {
            let skb = queue.dequeue().unwrap();
            assert_eq!(skb.data()[0], i as u8);
        }
    }

    #[test_case]
    fn test_queue_dequeue_empty() {
        let queue = SkBuffQueue::new();
        assert!(queue.dequeue().is_none());
    }

    #[test_case]
    fn test_queue_drain() {
        let queue = SkBuffQueue::new();

        // Enqueue several sk_buffs
        for i in 0..5 {
            let skb = test_skb(i);
            queue.enqueue(skb).unwrap();
        }

        assert_eq!(queue.len(), 5);

        // Drain all
        let drained: alloc::vec::Vec<_> = queue.drain().collect();
        assert_eq!(drained.len(), 5);
        assert!(queue.is_empty());
    }

    #[test_case]
    fn test_arc_refcounting() {
        let queue = SkBuffQueue::new();
        let skb = test_skb(42);

        // Initial refcount = 1
        assert_eq!(Arc::strong_count(&skb), 1);

        // Enqueue (Arc cloned into queue)
        queue.enqueue(skb.clone()).unwrap();
        assert_eq!(Arc::strong_count(&skb), 2);

        // Dequeue
        let dequeued = queue.dequeue().unwrap();
        assert_eq!(Arc::strong_count(&skb), 2);

        // Drop both
        drop(skb);
        assert_eq!(Arc::strong_count(&dequeued), 1);
        drop(dequeued);
        // skb memory freed here
    }
}

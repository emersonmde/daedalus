//! Lock-Free Packet Queue for Socket RX
//!
//! This module provides a Single-Producer Single-Consumer (SPSC) ring buffer
//! for packet reception. The interrupt handler (producer) enqueues packet references,
//! and the socket recv() (consumer) dequeues them.
//!
//! ## Design Pattern
//!
//! Follows the same pattern as the UART RX ring buffer (src/drivers/tty/serial/amba_pl011.rs):
//! - Power-of-2 capacity for efficient modulo via masking
//! - Atomic head/tail indices with Acquire/Release ordering
//! - Single producer (interrupt handler), single consumer (socket recv)
//! - No locks required - safe for interrupt context

use core::sync::atomic::{AtomicUsize, Ordering};

/// Packet reference in the queue
///
/// Instead of copying packet data, we store a reference to the packet pool buffer.
#[derive(Debug, Clone, Copy)]
pub struct PacketRef {
    /// Buffer ID in packet pool (0..256)
    pub buffer_id: usize,

    /// Offset into buffer where packet data starts
    pub offset: usize,

    /// Length of packet data
    pub length: usize,

    /// Receive timestamp (microseconds since boot)
    pub timestamp: u64,
}

/// Lock-free SPSC ring buffer for packet references
///
/// # Capacity
///
/// Fixed at 32 packet refs per socket (compile-time constant).
/// This handles bursts of ~1ms at typical packet rates (30k pps).
///
/// # Thread Safety
///
/// - **Single Producer**: Interrupt handler (router) enqueues packets
/// - **Single Consumer**: Socket recv() dequeues packets
/// - **Ordering**: Acquire/Release prevents reordering across boundaries
///
/// # Memory Ordering
///
/// Producer:
/// 1. Write packet to queue[head]
/// 2. Store head with Release (ensures write visible before advancing head)
///
/// Consumer:
/// 1. Load head with Acquire (ensures we see producer's writes)
/// 2. Read packet from queue[tail]
/// 3. Store tail with Release
pub struct RxPacketQueue {
    /// Ring buffer of packet references
    /// Note: One slot always remains empty to distinguish full from empty
    queue: [Option<PacketRef>; Self::CAPACITY],

    /// Write index (producer only)
    head: AtomicUsize,

    /// Read index (consumer only)
    tail: AtomicUsize,
}

impl RxPacketQueue {
    /// Queue capacity (MUST be power of 2 for efficient masking)
    pub const CAPACITY: usize = 32;

    /// Bit mask for wrapping indices (CAPACITY - 1)
    const MASK: usize = Self::CAPACITY - 1;

    /// Create a new empty queue
    pub const fn new() -> Self {
        const NONE: Option<PacketRef> = None;
        Self {
            queue: [NONE; Self::CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Enqueue a packet reference (producer)
    ///
    /// # Arguments
    /// * `packet` - Packet reference to enqueue
    ///
    /// # Returns
    /// * `Ok(())` if packet was enqueued
    /// * `Err(packet)` if queue is full (returns packet back to caller)
    ///
    /// # Safety
    /// Must only be called from single producer (interrupt handler).
    pub fn enqueue(&self, packet: PacketRef) -> Result<(), PacketRef> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        // Check if queue is full
        // Queue is full when (head + 1) % CAPACITY == tail
        let next_head = (head + 1) & Self::MASK;
        if next_head == tail {
            return Err(packet); // Queue full
        }

        // Write packet to queue
        // SAFETY: head < CAPACITY due to masking, and we're the only writer
        unsafe {
            let queue_ptr = self.queue.as_ptr() as *mut Option<PacketRef>;
            queue_ptr.add(head).write(Some(packet));
        }

        // Advance head with Release ordering (makes write visible)
        self.head.store(next_head, Ordering::Release);

        Ok(())
    }

    /// Dequeue a packet reference (consumer)
    ///
    /// # Returns
    /// * `Some(packet)` if packet was available
    /// * `None` if queue is empty
    ///
    /// # Safety
    /// Must only be called from single consumer (socket recv).
    pub fn dequeue(&self) -> Option<PacketRef> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        // Check if queue is empty
        if head == tail {
            return None;
        }

        // Read packet from queue
        // SAFETY: tail < CAPACITY due to masking, and we're the only reader
        let packet = unsafe {
            let queue_ptr = self.queue.as_ptr();
            queue_ptr.add(tail).read()
        };

        // Advance tail with Release ordering
        let next_tail = (tail + 1) & Self::MASK;
        self.tail.store(next_tail, Ordering::Release);

        packet
    }

    /// Get current queue depth (approximate, may be stale)
    ///
    /// This is safe to call from any context but the result may be outdated
    /// by the time it's used (due to concurrent enqueue/dequeue).
    ///
    /// # Returns
    /// Number of packets currently in queue (0..CAPACITY)
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

// SAFETY: RxPacketQueue can be shared between threads because:
// - Single producer modifies head, single consumer modifies tail (no data races)
// - Acquire/Release ordering prevents reordering across synchronization points
// - Queue indices are always in bounds due to power-of-2 masking
// - This is the same safety reasoning as the UART RxRingBuffer
unsafe impl Sync for RxPacketQueue {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_queue_new_empty() {
        let queue = RxPacketQueue::new();
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
        assert!(!queue.is_full());
    }

    #[test_case]
    fn test_queue_enqueue_dequeue() {
        let queue = RxPacketQueue::new();

        let pkt1 = PacketRef {
            buffer_id: 0,
            offset: 0,
            length: 64,
            timestamp: 1000,
        };

        // Enqueue
        assert!(queue.enqueue(pkt1).is_ok());
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());

        // Dequeue
        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.buffer_id, 0);
        assert_eq!(dequeued.length, 64);
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test_case]
    fn test_queue_full() {
        let queue = RxPacketQueue::new();

        // Fill queue (CAPACITY - 1 because one slot stays empty)
        for i in 0..(RxPacketQueue::CAPACITY - 1) {
            let pkt = PacketRef {
                buffer_id: i,
                offset: 0,
                length: 64,
                timestamp: i as u64,
            };
            assert!(queue.enqueue(pkt).is_ok());
        }

        assert!(queue.is_full());

        // Next enqueue should fail
        let pkt = PacketRef {
            buffer_id: 999,
            offset: 0,
            length: 64,
            timestamp: 9999,
        };
        assert!(queue.enqueue(pkt).is_err());
    }

    #[test_case]
    fn test_queue_wraparound() {
        let queue = RxPacketQueue::new();

        // Fill and drain multiple times
        for round in 0..3 {
            // Fill queue
            for i in 0..(RxPacketQueue::CAPACITY - 1) {
                let pkt = PacketRef {
                    buffer_id: round * 100 + i,
                    offset: 0,
                    length: 64,
                    timestamp: i as u64,
                };
                assert!(queue.enqueue(pkt).is_ok());
            }

            // Drain queue
            for i in 0..(RxPacketQueue::CAPACITY - 1) {
                let pkt = queue.dequeue().unwrap();
                assert_eq!(pkt.buffer_id, round * 100 + i);
            }

            assert!(queue.is_empty());
        }
    }

    #[test_case]
    fn test_queue_fifo_order() {
        let queue = RxPacketQueue::new();

        // Enqueue packets with different timestamps
        for i in 0..10 {
            let pkt = PacketRef {
                buffer_id: i,
                offset: 0,
                length: 64,
                timestamp: i as u64 * 1000,
            };
            queue.enqueue(pkt).unwrap();
        }

        // Dequeue should return in FIFO order
        for i in 0..10 {
            let pkt = queue.dequeue().unwrap();
            assert_eq!(pkt.buffer_id, i);
            assert_eq!(pkt.timestamp, i as u64 * 1000);
        }
    }

    #[test_case]
    fn test_queue_dequeue_empty() {
        let queue = RxPacketQueue::new();
        assert!(queue.dequeue().is_none());
    }
}

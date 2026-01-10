//! Socket Buffer (sk_buff) - Linux-inspired packet buffer
//!
//! This module implements a packet buffer structure inspired by Linux's sk_buff.
//! The sk_buff decouples packet data from DMA buffers, allowing independent
//! lifecycles and solving the packet pool exhaustion problem.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │              SkBuff (metadata)                   │
//! ├─────────────────────────────────────────────────┤
//! │  head: 0                                         │
//! │  data_ptr: 14  (after Ethernet header)           │
//! │  tail: 64                                        │
//! │  mac_header: Some(0)                             │
//! │  network_header: Some(14)                        │
//! │  transport_header: Some(34)                      │
//! └──────────────┬──────────────────────────────────┘
//!                │
//!                ▼
//! ┌─────────────────────────────────────────────────┐
//! │           Packet Data (heap buffer)             │
//! ├─────────────────────────────────────────────────┤
//! │ [Eth Hdr][IP Hdr][TCP Hdr][Payload]             │
//! │  ^        ^       ^                              │
//! │  head     data    transport_header               │
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! # Reference Counting
//!
//! SkBuff uses Arc for automatic reference counting, enabling:
//! - Broadcast/multicast delivery (multiple sockets share same packet)
//! - Automatic cleanup when last reference drops
//! - Thread-safe sharing
//!
//! # References
//!
//! - Linux kernel sk_buff: <https://docs.kernel.org/networking/skbuff.html>
//! - "The Path of a Packet Through the Linux Kernel" Section 3.2

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::fmt;

use crate::drivers::timer::SystemTimer;
use crate::net::ethernet::MacAddress;
use crate::net::socket::Protocol;

/// Maximum packet size (Ethernet MTU + headers)
/// 1500 (MTU) + 14 (Ethernet) + 20 (IP max with options) + 60 (TCP max with options)
pub const MAX_PACKET_SIZE: usize = 2048;

/// Socket buffer - Linux sk_buff equivalent
///
/// This structure owns packet data and associated metadata. It is allocated
/// on the heap and freed when the last Arc reference is dropped.
///
/// # Lifecycle
///
/// 1. **Ingress**: Allocated by interrupt handler, copied from DMA
/// 2. **Routing**: Passed to protocol handler via Arc
/// 3. **Queuing**: Enqueued to socket RX queue (Arc cloned)
/// 4. **Delivery**: Application reads, Arc dropped
///
/// # Example
///
/// ```ignore
/// // In GENET interrupt handler
/// let skb = SkBuff::from_dma(frame_data)?;
/// router::route_packet(skb);  // Arc passed by value
/// ```
pub struct SkBuff {
    /// Packet data (owned, heap-allocated)
    ///
    /// This is a boxed slice containing the complete packet including all headers.
    /// Using Box<[u8]> instead of Vec<u8> saves 8 bytes (no capacity field).
    data: Box<[u8]>,

    /// Metadata and header pointers
    ///
    /// These offsets point into the data buffer and track header boundaries.
    /// All offsets are relative to the start of the data buffer (index 0).
    headers: SkBuffHeaders,

    /// Packet metadata (extracted from headers)
    metadata: PacketMetadata,
}

/// sk_buff header pointers (offsets into data buffer)
///
/// These track the boundaries of each protocol layer's header.
/// Linux uses pointers; we use offsets for safety and simplicity.
#[derive(Debug, Clone)]
struct SkBuffHeaders {
    /// Start of buffer (always 0 for owned buffer)
    head: usize,

    /// Current layer's data start (moves as headers are pushed/pulled)
    data: usize,

    /// End of actual packet data
    tail: usize,

    /// Offset to MAC header (Ethernet)
    mac_header: Option<usize>,

    /// Offset to network header (IP)
    network_header: Option<usize>,

    /// Offset to transport header (TCP/UDP)
    transport_header: Option<usize>,
}

/// Packet metadata extracted during reception
///
/// This is populated by protocol handlers as the packet traverses the stack.
#[derive(Debug, Clone)]
pub struct PacketMetadata {
    /// Protocol (from Ethernet EtherType or IP protocol field)
    pub protocol: Protocol,

    /// Source MAC address (from Ethernet header)
    pub src_mac: Option<MacAddress>,

    /// Destination MAC address (from Ethernet header)
    pub dst_mac: Option<MacAddress>,

    /// Source IP address (from IP header, future)
    pub src_ip: Option<[u8; 4]>,

    /// Destination IP address (from IP header, future)
    pub dst_ip: Option<[u8; 4]>,

    /// Source port (from TCP/UDP header, future)
    pub src_port: Option<u16>,

    /// Destination port (from TCP/UDP header, future)
    pub dst_port: Option<u16>,

    /// Receive timestamp (microseconds since boot)
    pub timestamp_us: u64,
}

/// Errors that can occur during sk_buff operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkBuffError {
    /// Heap allocation failed (out of memory)
    AllocationFailed,

    /// Packet size exceeds maximum
    PacketTooLarge,

    /// Invalid packet data (e.g., truncated headers)
    InvalidPacket,

    /// Not enough headroom to push header
    InsufficientHeadroom,

    /// Not enough data to pull header
    InsufficientData,
}

impl fmt::Display for SkBuffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "heap allocation failed"),
            Self::PacketTooLarge => write!(f, "packet exceeds maximum size"),
            Self::InvalidPacket => write!(f, "invalid packet data"),
            Self::InsufficientHeadroom => write!(f, "insufficient headroom for header"),
            Self::InsufficientData => write!(f, "insufficient data to pull header"),
        }
    }
}

impl SkBuff {
    /// Allocate sk_buff from DMA frame data (copies data)
    ///
    /// This is the primary allocation point for ingress packets. The packet data
    /// is **copied** from the DMA buffer to a new heap buffer, allowing the DMA
    /// descriptor to be freed immediately.
    ///
    /// # Arguments
    ///
    /// * `frame_data` - Slice from GENET DMA buffer (will be copied)
    ///
    /// # Returns
    ///
    /// * `Ok(Arc<SkBuff>)` - New sk_buff with copied data
    /// * `Err(SkBuffError::PacketTooLarge)` - Packet exceeds MAX_PACKET_SIZE
    /// * `Err(SkBuffError::AllocationFailed)` - Heap allocation failed
    ///
    /// # Safety
    ///
    /// This function allocates on the heap. If heap is exhausted, returns error.
    /// Caller must handle error gracefully (drop packet, log, continue).
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In GENET interrupt handler
    /// let skb = match SkBuff::from_dma(frame_data) {
    ///     Ok(skb) => skb,
    ///     Err(_) => {
    ///         STATS.heap_exhausted.fetch_add(1, Ordering::Relaxed);
    ///         genet.free_rx_buffer();
    ///         continue;  // Drop packet
    ///     }
    /// };
    ///
    /// // DMA descriptor can be freed immediately (data copied)
    /// genet.free_rx_buffer();
    /// ```
    pub fn from_dma(frame_data: &[u8]) -> Result<Arc<Self>, SkBuffError> {
        if frame_data.len() > MAX_PACKET_SIZE {
            return Err(SkBuffError::PacketTooLarge);
        }

        // Allocate and copy packet data to heap
        // NOTE: This may fail if heap is exhausted
        let data: Box<[u8]> = frame_data.to_vec().into_boxed_slice();

        let len = data.len();

        let headers = SkBuffHeaders {
            head: 0,
            data: 0, // Initially points to start
            tail: len,
            mac_header: None,       // Will be set by Ethernet parser
            network_header: None,   // Will be set by IP parser
            transport_header: None, // Will be set by TCP/UDP parser
        };

        let metadata = PacketMetadata {
            protocol: Protocol::None, // Will be set by protocol handler
            src_mac: None,
            dst_mac: None,
            src_ip: None,
            dst_ip: None,
            src_port: None,
            dst_port: None,
            timestamp_us: SystemTimer::timestamp_us(),
        };

        Ok(Arc::new(Self {
            data,
            headers,
            metadata,
        }))
    }

    /// Get current layer's data (after headers have been pulled)
    ///
    /// Returns a slice from the current data pointer to tail.
    /// This is what protocol handlers see when parsing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Parse Ethernet header
    /// let eth_frame = EthernetFrame::parse(skb.data())?;
    ///
    /// // Pull Ethernet header (move data pointer forward)
    /// skb.pull_header(14)?;
    ///
    /// // Now skb.data() points to IP header
    /// let ip_packet = IpPacket::parse(skb.data())?;
    /// ```
    pub fn data(&self) -> &[u8] {
        &self.data[self.headers.data..self.headers.tail]
    }

    /// Pull header (move data pointer forward, strip header)
    ///
    /// This is called by each protocol layer after parsing its header.
    /// Moves the data pointer forward by `size` bytes, effectively
    /// "consuming" the header.
    ///
    /// # Arguments
    ///
    /// * `size` - Number of bytes to pull (header size)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Header pulled successfully
    /// * `Err(SkBuffError::InsufficientData)` - Not enough data to pull
    ///
    /// # Example
    ///
    /// ```ignore
    /// // After parsing Ethernet header (14 bytes)
    /// skb.pull_header(14)?;
    /// // Now skb.data() points to IP header
    /// ```
    pub fn pull_header(&mut self, size: usize) -> Result<(), SkBuffError> {
        if self.headers.data + size > self.headers.tail {
            return Err(SkBuffError::InsufficientData);
        }

        self.headers.data += size;
        Ok(())
    }

    /// Push header (move data pointer backward, reserve space for header)
    ///
    /// This is called by each protocol layer when building a packet (egress).
    /// Moves the data pointer backward by `size` bytes, creating space for
    /// a new header. Returns a mutable slice where the header can be written.
    ///
    /// # Arguments
    ///
    /// * `size` - Number of bytes to reserve (header size)
    ///
    /// # Returns
    ///
    /// * `Ok(&mut [u8])` - Mutable slice for writing header
    /// * `Err(SkBuffError::InsufficientHeadroom)` - Not enough space before data
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Reserve space for Ethernet header (14 bytes)
    /// let eth_hdr = skb.push_header(14)?;
    /// // Write Ethernet header
    /// eth_hdr[0..6].copy_from_slice(&dst_mac.octets());
    /// eth_hdr[6..12].copy_from_slice(&src_mac.octets());
    /// eth_hdr[12..14].copy_from_slice(&ethertype.to_be_bytes());
    /// ```
    pub fn push_header(&mut self, size: usize) -> Result<&mut [u8], SkBuffError> {
        if self.headers.data < size {
            return Err(SkBuffError::InsufficientHeadroom);
        }

        self.headers.data -= size;
        let start = self.headers.data;
        let end = start + size;

        Ok(&mut self.data[start..end])
    }

    /// Set MAC header offset
    ///
    /// Called by Ethernet parser to mark the start of the MAC header.
    pub fn set_mac_header(&mut self, offset: usize) {
        self.headers.mac_header = Some(offset);
    }

    /// Set network header offset
    ///
    /// Called by IP parser to mark the start of the network header.
    pub fn set_network_header(&mut self, offset: usize) {
        self.headers.network_header = Some(offset);
    }

    /// Set transport header offset
    ///
    /// Called by TCP/UDP parser to mark the start of the transport header.
    pub fn set_transport_header(&mut self, offset: usize) {
        self.headers.transport_header = Some(offset);
    }

    /// Get MAC header offset
    pub fn mac_header(&self) -> Option<usize> {
        self.headers.mac_header
    }

    /// Get network header offset
    pub fn network_header(&self) -> Option<usize> {
        self.headers.network_header
    }

    /// Get transport header offset
    pub fn transport_header(&self) -> Option<usize> {
        self.headers.transport_header
    }

    /// Get mutable reference to metadata
    ///
    /// Allows protocol handlers to update metadata as they parse headers.
    pub fn metadata_mut(&mut self) -> &mut PacketMetadata {
        &mut self.metadata
    }

    /// Get reference to metadata
    pub fn metadata(&self) -> &PacketMetadata {
        &self.metadata
    }

    /// Get total packet length (all headers + payload)
    pub fn len(&self) -> usize {
        self.headers.tail - self.headers.head
    }

    /// Check if packet is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get raw packet data (all headers + payload)
    ///
    /// Returns the entire packet buffer, from head to tail.
    /// This is used when transmitting the complete frame.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[self.headers.head..self.headers.tail]
    }
}

// Arc<SkBuff> already provides clone_ref via Arc::clone()
// No need for explicit implementation

impl fmt::Debug for SkBuff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkBuff")
            .field("len", &self.len())
            .field("data_ptr", &self.headers.data)
            .field("mac_header", &self.headers.mac_header)
            .field("network_header", &self.headers.network_header)
            .field("transport_header", &self.headers.transport_header)
            .field("protocol", &self.metadata.protocol)
            .field("timestamp", &self.metadata.timestamp_us)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test_case]
    fn test_skbuff_allocation() {
        let data = [0u8; 64];
        let skb = SkBuff::from_dma(&data).expect("allocation failed");

        assert_eq!(skb.len(), 64);
        assert_eq!(skb.data().len(), 64);
        assert!(!skb.is_empty());
    }

    #[test_case]
    fn test_skbuff_too_large() {
        let data = vec![0u8; MAX_PACKET_SIZE + 1];
        let result = SkBuff::from_dma(&data);

        assert!(matches!(result, Err(SkBuffError::PacketTooLarge)));
    }

    #[test_case]
    fn test_pull_header() {
        let data = [0u8; 64];
        let skb_arc = SkBuff::from_dma(&data).unwrap();
        let mut skb = Arc::try_unwrap(skb_arc).unwrap(); // Get ownership for mutation

        // Pull Ethernet header (14 bytes)
        skb.pull_header(14).expect("pull failed");
        assert_eq!(skb.data().len(), 50);

        // Pull IP header (20 bytes)
        skb.pull_header(20).expect("pull failed");
        assert_eq!(skb.data().len(), 30);
    }

    #[test_case]
    fn test_pull_header_insufficient_data() {
        let data = [0u8; 10];
        let skb_arc = SkBuff::from_dma(&data).unwrap();
        let mut skb = Arc::try_unwrap(skb_arc).unwrap();

        let result = skb.pull_header(20);
        assert!(matches!(result, Err(SkBuffError::InsufficientData)));
    }

    #[test_case]
    fn test_push_header() {
        let data = vec![0u8; 100];
        let skb_arc = SkBuff::from_dma(&data).unwrap();
        let mut skb = Arc::try_unwrap(skb_arc).unwrap();

        // Simulate egress: pull to create headroom
        skb.pull_header(50).expect("pull failed");
        assert_eq!(skb.data().len(), 50);

        // Now push Ethernet header
        let eth_hdr = skb.push_header(14).expect("push failed");
        assert_eq!(eth_hdr.len(), 14);
        assert_eq!(skb.data().len(), 64); // 14 + 50
    }

    #[test_case]
    fn test_arc_refcounting() {
        let data = [0u8; 64];
        let skb1 = SkBuff::from_dma(&data).unwrap();

        // Clone Arc (increment refcount)
        let skb2 = Arc::clone(&skb1);

        assert_eq!(Arc::strong_count(&skb1), 2);
        assert_eq!(Arc::strong_count(&skb2), 2);

        drop(skb2);
        assert_eq!(Arc::strong_count(&skb1), 1);
    }

    #[test_case]
    fn test_header_offsets() {
        let data = [0u8; 64];
        let skb_arc = SkBuff::from_dma(&data).unwrap();
        let mut skb = Arc::try_unwrap(skb_arc).unwrap();

        skb.set_mac_header(0);
        skb.set_network_header(14);
        skb.set_transport_header(34);

        assert_eq!(skb.mac_header(), Some(0));
        assert_eq!(skb.network_header(), Some(14));
        assert_eq!(skb.transport_header(), Some(34));
    }
}

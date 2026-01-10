//! ARP Protocol Handler
//!
//! Implements ProtocolHandler trait for ARP (Address Resolution Protocol).
//! Handles incoming ARP packets and routes them to bound sockets.

use crate::net::protocol::{ProtocolError, ProtocolHandler};
use crate::net::skbuff::SkBuff;
use crate::net::socket::{Protocol, SOCKET_TABLE, Socket, SocketAddr, SocketKey};
use alloc::sync::Arc;

/// ARP protocol handler
///
/// Handles incoming ARP packets by looking up sockets bound to AF_PACKET + ARP.
/// Currently supports single socket (arp-probe diagnostic).
/// Future: Could support multiple sockets for monitoring/debugging.
pub struct ArpProtocol;

impl ArpProtocol {
    /// Create new ARP protocol handler
    pub const fn new() -> Self {
        Self
    }
}

impl ProtocolHandler for ArpProtocol {
    fn protocol(&self) -> Protocol {
        Protocol::Arp
    }

    fn receive(&self, skb: Arc<SkBuff>) -> Result<(), ProtocolError> {
        // Increment global ARP RX counter (for statistics)
        arp_rx_count_increment();

        // Look up socket bound to ARP protocol
        let sock = self.lookup_socket(&skb).ok_or(ProtocolError::NoSocket)?;

        // Enqueue to socket's RX queue (release lock before any potential print)
        let (queue_depth, enqueue_result) = {
            let table = SOCKET_TABLE.lock();
            let socket = table.get(sock).ok_or(ProtocolError::InvalidSocket)?;

            let depth_before = socket.rx_queue.len();
            let result = socket.rx_queue.enqueue(skb);
            (depth_before, result)
        }; // Lock dropped here

        match enqueue_result {
            Ok(()) => {
                // Log only first ARP packet for initial verification
                if arp_rx_count() == 1 {
                    crate::println!(
                        "[ARP] Packet #1 delivered to socket (queue depth was {})",
                        queue_depth
                    );
                }
                Ok(())
            }
            Err(_) => Err(ProtocolError::QueueFull),
        }
    }

    fn send(&self, _skb: Arc<SkBuff>, _dst: &SocketAddr) -> Result<(), ProtocolError> {
        // ARP uses sendto() directly (builds frame in userspace)
        // No need for protocol-layer send support
        Err(ProtocolError::NotSupported)
    }

    fn lookup_socket(&self, _skb: &SkBuff) -> Option<Socket> {
        // Simple lookup: Find socket bound to ARP protocol
        // For AF_PACKET sockets, binding is by EtherType only
        let key = SocketKey::packet(Protocol::Arp);

        let table = SOCKET_TABLE.lock();
        table.lookup(&key).map(Socket::from_id)
    }
}

/// Global ARP RX packet counter
static ARP_RX_COUNT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Increment ARP RX counter and return new count
fn arp_rx_count_increment() -> u32 {
    ARP_RX_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed) + 1
}

/// Get current ARP RX packet count
pub fn arp_rx_count() -> u32 {
    ARP_RX_COUNT.load(core::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::socket::table::SocketTable;
    use crate::net::socket::types::{AddressFamily, SocketType};

    #[test_case]
    fn test_arp_protocol_lookup_no_socket() {
        let handler = ArpProtocol::new();

        // Create dummy sk_buff
        let data = alloc::vec![0u8; 64];
        let skb = SkBuff::from_dma(&data).unwrap();

        // Lookup should fail (no socket bound)
        assert!(handler.lookup_socket(&skb).is_none());
    }

    #[test_case]
    fn test_arp_protocol_lookup_with_socket() {
        use crate::net::socket::types::SocketAddr;

        // Create socket table and bind ARP socket
        let mut table = SocketTable::new();
        let sock = table
            .create(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)
            .unwrap();
        let addr = SocketAddr::packet(0x0806); // ARP EtherType
        table.bind(sock, addr).unwrap();

        // Temporarily replace global table for test
        // (Note: This test has limitations due to global state)
        // In practice, would use dependency injection for testability

        let _handler = ArpProtocol::new();
        let data = alloc::vec![0u8; 64];
        let _skb = SkBuff::from_dma(&data).unwrap();

        // Lookup should succeed if socket bound in global table
        // (This test is illustrative - actual behavior depends on global state)
    }
}

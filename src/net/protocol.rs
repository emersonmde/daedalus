//! Protocol Handler Registry - Linux-style protocol dispatch
//!
//! This module provides a trait-based protocol handler system, inspired by Linux's
//! `inet_register_protosw()` and protocol-specific receive functions.
//!
//! ## Architecture (from Linux paper Section 4.1.1 & 4.2.2)
//!
//! ```text
//! Router → Protocol Registry → Specific Handler (ARP, ICMP, UDP, TCP)
//!                                        ↓
//!                               Socket Lookup & Enqueue
//! ```
//!
//! Each protocol implements the `ProtocolHandler` trait with:
//! - `receive()`: Process incoming packet (ingress path)
//! - `send()`: Transmit outgoing packet (egress path)
//! - `lookup_socket()`: Find destination socket for packet
//!
//! ## Comparison to Current Router
//!
//! **Before**: Hardcoded `match ethertype { ETHERTYPE_ARP => route_arp(...) }`
//! **After**: `registry.get(protocol).receive(skb)`
//!
//! This enables adding new protocols (ICMP, UDP, TCP) without modifying the router.

use crate::net::skbuff::SkBuff;
use crate::net::socket::{Protocol, Socket, SocketAddr, SocketError};
use crate::sync::Mutex;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;

/// Protocol handler trait (equivalent to Linux's proto_ops + inet_protosw)
///
/// Each network protocol (ARP, ICMP, UDP, TCP) implements this trait.
/// Handlers are registered in the global registry during kernel init.
///
/// # Example
///
/// ```ignore
/// pub struct ArpProtocol;
///
/// impl ProtocolHandler for ArpProtocol {
///     fn protocol(&self) -> Protocol {
///         Protocol::Arp
///     }
///
///     fn receive(&self, skb: Arc<SkBuff>) -> Result<(), ProtocolError> {
///         // Parse ARP packet, find socket, enqueue
///         let sock = self.lookup_socket(&skb)?;
///         SOCKET_TABLE.lock().get(sock)?.rx_queue.enqueue(skb)
///     }
///
///     // ... other methods
/// }
/// ```
pub trait ProtocolHandler: Send + Sync {
    /// Protocol identifier (ARP, ICMP, UDP, TCP, etc.)
    fn protocol(&self) -> Protocol;

    /// Handle incoming packet (ingress path)
    ///
    /// Called by router when packet arrives for this protocol.
    ///
    /// # Arguments
    /// * `skb` - Socket buffer containing packet data
    ///
    /// # Returns
    /// * `Ok(())` if packet was successfully delivered to socket
    /// * `Err(ProtocolError)` if delivery failed (no socket, queue full, etc.)
    ///
    /// # Example
    /// ```ignore
    /// fn receive(&self, skb: Arc<SkBuff>) -> Result<(), ProtocolError> {
    ///     let sock = self.lookup_socket(&skb)?;
    ///     SOCKET_TABLE.lock()
    ///         .get(sock)?
    ///         .rx_queue.enqueue(skb)
    ///         .map_err(|_| ProtocolError::QueueFull)
    /// }
    /// ```
    fn receive(&self, skb: Arc<SkBuff>) -> Result<(), ProtocolError>;

    /// Send packet (egress path, future)
    ///
    /// Called by socket send() to transmit packet.
    ///
    /// # Arguments
    /// * `skb` - Socket buffer to transmit
    /// * `dst` - Destination address
    ///
    /// # Returns
    /// * `Ok(())` if packet was queued for transmission
    /// * `Err(ProtocolError)` if transmission failed
    ///
    /// # Note
    /// For Milestone #14 (ARP only), this is unused.
    /// Milestone #15+ will use this for UDP/TCP TX path.
    fn send(&self, _skb: Arc<SkBuff>, _dst: &SocketAddr) -> Result<(), ProtocolError> {
        // Default implementation: not supported
        Err(ProtocolError::NotSupported)
    }

    /// Look up socket for incoming packet
    ///
    /// Extracts addressing information from sk_buff and finds matching socket.
    ///
    /// # Arguments
    /// * `skb` - Socket buffer with packet metadata
    ///
    /// # Returns
    /// * `Some(Socket)` if matching socket found
    /// * `None` if no socket bound to this protocol/address
    ///
    /// # Example
    /// ```ignore
    /// fn lookup_socket(&self, skb: &SkBuff) -> Option<Socket> {
    ///     let key = SocketKey::packet(Protocol::Arp);
    ///     SOCKET_TABLE.lock().lookup(&key).map(Socket)
    /// }
    /// ```
    fn lookup_socket(&self, skb: &SkBuff) -> Option<Socket>;
}

/// Protocol handler errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    /// No socket bound to receive packet
    NoSocket,

    /// Socket queue full, packet dropped
    QueueFull,

    /// Malformed packet (parse error)
    MalformedPacket,

    /// Protocol operation not supported
    NotSupported,

    /// Invalid socket handle
    InvalidSocket,
}

impl From<SocketError> for ProtocolError {
    fn from(err: SocketError) -> Self {
        match err {
            SocketError::InvalidSocket => ProtocolError::InvalidSocket,
            _ => ProtocolError::NotSupported,
        }
    }
}

/// Protocol registry (global, initialized at boot)
///
/// Maps Protocol enum to trait objects implementing ProtocolHandler.
/// Equivalent to Linux's `inet_protos` array and `inet_protosw` list.
///
/// # Usage
///
/// ```ignore
/// // Register protocol (in kernel init)
/// PROTOCOL_REGISTRY.lock().register(Arc::new(ArpProtocol));
///
/// // Dispatch incoming packet (in router)
/// if let Some(handler) = PROTOCOL_REGISTRY.lock().get(&Protocol::Arp) {
///     handler.receive(skb)?;
/// }
/// ```
pub struct ProtocolRegistry {
    /// Protocol handlers (BTreeMap for deterministic iteration)
    handlers: BTreeMap<Protocol, Arc<dyn ProtocolHandler>>,
}

impl ProtocolRegistry {
    /// Create empty registry
    pub const fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }

    /// Register protocol handler
    ///
    /// # Arguments
    /// * `handler` - Protocol handler implementation
    ///
    /// # Panics
    /// Panics if protocol already registered (indicates programming error).
    ///
    /// # Example
    /// ```ignore
    /// registry.register(Arc::new(ArpProtocol));
    /// registry.register(Arc::new(UdpProtocol));
    /// ```
    pub fn register(&mut self, handler: Arc<dyn ProtocolHandler>) {
        let protocol = handler.protocol();
        if self.handlers.insert(protocol, handler).is_some() {
            panic!("Protocol {:?} already registered", protocol);
        }
    }

    /// Get handler for protocol
    ///
    /// # Arguments
    /// * `protocol` - Protocol to look up
    ///
    /// # Returns
    /// * `Some(&handler)` if protocol registered
    /// * `None` if protocol not supported
    ///
    /// # Example
    /// ```ignore
    /// if let Some(handler) = registry.get(&Protocol::Arp) {
    ///     handler.receive(skb)?;
    /// } else {
    ///     // Drop packet - protocol not supported
    /// }
    /// ```
    pub fn get(&self, protocol: &Protocol) -> Option<&Arc<dyn ProtocolHandler>> {
        self.handlers.get(protocol)
    }

    /// Get number of registered protocols
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

/// Global protocol registry
///
/// Initialized during kernel boot via `init_protocols()`.
pub static PROTOCOL_REGISTRY: Mutex<ProtocolRegistry> = Mutex::new(ProtocolRegistry::new());

/// Initialize protocol handlers (called during kernel init)
///
/// Registers all supported protocols. Currently only ARP is supported.
/// Future milestones will add ICMP, UDP, TCP handlers.
///
/// # Example Boot Sequence
///
/// ```ignore
/// pub fn kernel_init() {
///     // ... hardware init ...
///     net::protocol::init_protocols();  // Register ARP, ICMP, UDP, TCP
///     // ... other subsystems ...
/// }
/// ```
pub fn init_protocols() {
    use crate::net::protocols::ArpProtocol;

    let mut registry = PROTOCOL_REGISTRY.lock();

    // Register ARP protocol handler
    registry.register(Arc::new(ArpProtocol::new()));

    let count = registry.len();
    drop(registry); // Release lock before printing

    crate::println!(
        "[PROTOCOL] Protocol registry initialized ({} protocols)",
        count
    );
}

/// Get number of registered protocols
///
/// Returns the count of protocol handlers currently registered in the global registry.
pub fn registered_protocol_count() -> usize {
    PROTOCOL_REGISTRY.lock().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock protocol for testing
    struct MockProtocol {
        proto: Protocol,
    }

    impl ProtocolHandler for MockProtocol {
        fn protocol(&self) -> Protocol {
            self.proto
        }

        fn receive(&self, _skb: Arc<SkBuff>) -> Result<(), ProtocolError> {
            Ok(())
        }

        fn lookup_socket(&self, _skb: &SkBuff) -> Option<Socket> {
            None
        }
    }

    #[test_case]
    fn test_registry_register_and_lookup() {
        let mut registry = ProtocolRegistry::new();

        // Register ARP handler
        let arp_handler = Arc::new(MockProtocol {
            proto: Protocol::Arp,
        });
        registry.register(arp_handler.clone());

        // Lookup should succeed
        assert!(registry.get(&Protocol::Arp).is_some());
        assert_eq!(registry.len(), 1);

        // Unknown protocol should fail
        assert!(registry.get(&Protocol::Icmp).is_none());
    }
}

//! Socket Table - Socket lifecycle and lookup management
//!
//! This module manages socket allocation, binding, and lookup. It provides
//! fast O(1) lookup by (protocol, IP, port) for packet routing.

use super::queue::SkBuffQueue;
use super::types::{AddressFamily, Protocol, Socket, SocketAddr, SocketError, SocketType};
use crate::sync::Mutex;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Maximum number of sockets
const MAX_SOCKETS: usize = 64;

/// Socket lookup key
///
/// Used to find which socket should receive a packet.
/// For AF_PACKET: (protocol, ethertype, 0)
/// For AF_INET: (protocol, IP, port)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SocketKey {
    /// Protocol (Arp, Udp, Tcp, etc.)
    pub protocol: Protocol,

    /// Local IP address (None = wildcard/INADDR_ANY)
    pub local_ip: Option<[u8; 4]>,

    /// Local port (None = no port, e.g., for ICMP or raw IP)
    pub local_port: Option<u16>,
}

impl SocketKey {
    /// Create key for AF_PACKET socket
    pub fn packet(protocol: Protocol) -> Self {
        Self {
            protocol,
            local_ip: None,
            local_port: None,
        }
    }

    /// Create key for AF_INET raw socket (ICMP, raw IP)
    pub fn inet_raw(protocol: Protocol, ip: Option<[u8; 4]>) -> Self {
        Self {
            protocol,
            local_ip: ip,
            local_port: None,
        }
    }

    /// Create key for AF_INET socket with port (UDP, TCP)
    pub fn inet_port(protocol: Protocol, ip: Option<[u8; 4]>, port: u16) -> Self {
        Self {
            protocol,
            local_ip: ip,
            local_port: Some(port),
        }
    }
}

/// Socket state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    /// Socket created but not bound
    Created,

    /// Socket bound to address
    Bound,

    /// Socket connected (future, for TCP)
    Connected,

    /// Socket closed
    Closed,
}

/// Internal socket implementation
pub struct SocketImpl {
    /// Socket ID (index in table)
    pub id: usize,

    /// Address family
    pub family: AddressFamily,

    /// Socket type
    pub socket_type: SocketType,

    /// Protocol
    pub protocol: Protocol,

    /// Current state
    pub state: SocketState,

    /// Bound address (None if not bound)
    pub bind_addr: Option<SocketAddr>,

    /// Remote address (future, for connected sockets)
    pub remote_addr: Option<SocketAddr>,

    /// Receive queue (ingress - packets from network)
    pub rx_queue: SkBuffQueue,

    /// Transmit queue (egress - packets to network)
    pub tx_queue: SkBuffQueue,

    /// Maximum queue size
    pub max_queue_size: usize,
}

impl SocketImpl {
    /// Default RX queue size
    const DEFAULT_QUEUE_SIZE: usize = SkBuffQueue::CAPACITY;

    /// Create a new socket
    pub fn new(
        id: usize,
        family: AddressFamily,
        socket_type: SocketType,
        protocol: Protocol,
    ) -> Self {
        Self {
            id,
            family,
            socket_type,
            protocol,
            state: SocketState::Created,
            bind_addr: None,
            remote_addr: None,
            rx_queue: SkBuffQueue::new(),
            tx_queue: SkBuffQueue::new(),
            max_queue_size: Self::DEFAULT_QUEUE_SIZE,
        }
    }

    /// Check if this socket matches the given filter criteria
    ///
    /// Used by router to determine which sockets should receive a packet.
    pub fn matches(&self, key: &SocketKey) -> bool {
        // Protocol must match
        if self.protocol != key.protocol {
            return false;
        }

        // Check address matching based on binding
        match &self.bind_addr {
            None => false, // Not bound, can't receive
            Some(SocketAddr::Packet { protocol, .. }) => {
                // For AF_PACKET, check EtherType
                key.protocol.ethertype() == Some(*protocol)
            }
            Some(SocketAddr::Inet { ip, port }) => {
                // Check IP match (wildcard or exact)
                let ip_matches = match key.local_ip {
                    None => true, // Wildcard always matches
                    Some(key_ip) => {
                        // Check if socket bound to wildcard or exact IP
                        *ip == [0, 0, 0, 0] || *ip == key_ip
                    }
                };

                // Check port match
                let port_matches = match key.local_port {
                    None => *port == 0, // No port (ICMP, raw IP)
                    Some(key_port) => *port == 0 || *port == key_port,
                };

                ip_matches && port_matches
            }
        }
    }
}

/// Socket table
pub struct SocketTable {
    /// Array of sockets (index = socket ID)
    pub(crate) sockets: Vec<Option<SocketImpl>>,

    /// Binding map: SocketKey → Socket ID
    /// Used for fast lookup during packet routing
    bindings: BTreeMap<SocketKey, usize>,

    /// Next ephemeral port to allocate (for bind with port=0)
    next_ephemeral: u16,

    /// Number of bound sockets (reference count for GIC interrupt enable)
    bound_count: usize,
}

impl SocketTable {
    /// Minimum ephemeral port (IANA recommendation: 49152)
    const EPHEMERAL_MIN: u16 = 49152;

    /// Maximum ephemeral port
    const EPHEMERAL_MAX: u16 = 65535;

    /// Create a new empty socket table
    pub fn new() -> Self {
        Self {
            sockets: Vec::new(),
            bindings: BTreeMap::new(),
            next_ephemeral: Self::EPHEMERAL_MIN,
            bound_count: 0,
        }
    }

    /// Create a new socket
    ///
    /// # Arguments
    /// * `family` - Address family (Packet or Inet)
    /// * `socket_type` - Socket type (Raw, Dgram, Stream)
    /// * `protocol` - Protocol (Arp, Udp, Tcp, etc.)
    ///
    /// # Returns
    /// Socket handle on success, error if too many sockets or invalid parameters.
    pub fn create(
        &mut self,
        family: AddressFamily,
        socket_type: SocketType,
        protocol: Protocol,
    ) -> Result<Socket, SocketError> {
        // Validate family/type/protocol combination
        match (family, socket_type, protocol) {
            // AF_PACKET + SOCK_RAW + ARP = valid
            (AddressFamily::Packet, SocketType::Raw, Protocol::Arp) => {}

            // AF_INET + SOCK_RAW + ICMP = valid (future)
            (AddressFamily::Inet, SocketType::Raw, Protocol::Icmp) => {}

            // AF_INET + SOCK_DGRAM + UDP = valid (future)
            (AddressFamily::Inet, SocketType::Dgram, Protocol::Udp) => {}

            // AF_INET + SOCK_STREAM + TCP = valid (future)
            (AddressFamily::Inet, SocketType::Stream, Protocol::Tcp) => {}

            // Everything else is invalid
            _ => return Err(SocketError::ProtocolNotSupported),
        }

        // Find free slot
        let id = match self.sockets.iter().position(|s| s.is_none()) {
            Some(idx) => idx,
            None => {
                // No free slot, allocate new if under limit
                if self.sockets.len() >= MAX_SOCKETS {
                    return Err(SocketError::TooManySockets);
                }
                let idx = self.sockets.len();
                self.sockets.push(None);
                idx
            }
        };

        // Create socket
        let socket = SocketImpl::new(id, family, socket_type, protocol);
        self.sockets[id] = Some(socket);

        Ok(Socket(id))
    }

    /// Bind socket to address
    ///
    /// # Arguments
    /// * `sock` - Socket handle
    /// * `addr` - Address to bind to
    ///
    /// # Returns
    /// Ok(()) on success, error if address in use or socket invalid.
    pub fn bind(&mut self, sock: Socket, addr: SocketAddr) -> Result<(), SocketError> {
        // First, validate socket and extract needed info
        let socket_protocol = {
            let socket = self
                .sockets
                .get(sock.0)
                .and_then(|s| s.as_ref())
                .ok_or(SocketError::InvalidSocket)?;

            // Check if already bound
            if socket.state != SocketState::Created {
                return Err(SocketError::InvalidArgument);
            }

            // Validate address family matches socket
            match (&socket.family, &addr) {
                (AddressFamily::Packet, SocketAddr::Packet { .. }) => {}
                (AddressFamily::Inet, SocketAddr::Inet { .. }) => {}
                _ => return Err(SocketError::InvalidAddressFamily),
            }

            socket.protocol
        }; // Drop socket borrow here

        // Allocate ephemeral port if needed (no socket borrow here)
        let bind_addr = match addr {
            SocketAddr::Packet { .. } => addr.clone(),
            SocketAddr::Inet { ip, port } => {
                let final_port = if port == 0 {
                    self.alloc_ephemeral_port()?
                } else {
                    port
                };
                SocketAddr::Inet {
                    ip,
                    port: final_port,
                }
            }
        };

        // Create binding key
        let key = match &bind_addr {
            SocketAddr::Packet { .. } => SocketKey::packet(socket_protocol),
            SocketAddr::Inet { ip, port } => {
                let ip_opt = if *ip == [0, 0, 0, 0] { None } else { Some(*ip) };
                if *port == 0 {
                    SocketKey::inet_raw(socket_protocol, ip_opt)
                } else {
                    SocketKey::inet_port(socket_protocol, ip_opt, *port)
                }
            }
        };

        // Check if address already in use
        if self.bindings.contains_key(&key) {
            return Err(SocketError::AddressInUse);
        }

        // Finally, bind socket (new borrow)
        let socket = self
            .sockets
            .get_mut(sock.0)
            .and_then(|s| s.as_mut())
            .ok_or(SocketError::InvalidSocket)?;

        self.bindings.insert(key, sock.0);
        socket.bind_addr = Some(bind_addr);
        socket.state = SocketState::Bound;

        // Track bound socket count (used to enable/disable GIC interrupt)
        self.bound_count += 1;

        Ok(())
    }

    /// Close socket and free resources
    ///
    /// # Arguments
    /// * `sock` - Socket handle
    pub fn close(&mut self, sock: Socket) -> Result<(), SocketError> {
        let socket = self
            .sockets
            .get_mut(sock.0)
            .and_then(|s| s.as_mut())
            .ok_or(SocketError::InvalidSocket)?;

        // Remove from bindings (track if socket was bound)
        let was_bound = if let Some(addr) = &socket.bind_addr {
            let key = match addr {
                SocketAddr::Packet { .. } => SocketKey::packet(socket.protocol),
                SocketAddr::Inet { ip, port } => {
                    let ip_opt = if *ip == [0, 0, 0, 0] { None } else { Some(*ip) };
                    if *port == 0 {
                        SocketKey::inet_raw(socket.protocol, ip_opt)
                    } else {
                        SocketKey::inet_port(socket.protocol, ip_opt, *port)
                    }
                }
            };
            self.bindings.remove(&key);
            true
        } else {
            false
        };

        // Drain both RX and TX queues
        // This prevents sk_buff leaks when socket is closed with pending packets
        // Arc refcounting ensures sk_buffs are freed when dropped
        for _skb in socket.rx_queue.drain() {
            // Drain silently
        }
        for _skb in socket.tx_queue.drain() {
            // Drain silently
        }

        // Mark socket as closed and free slot
        socket.state = SocketState::Closed;
        self.sockets[sock.0] = None;

        // Update bound count (used to disable GIC interrupt outside lock)
        if was_bound {
            self.bound_count = self.bound_count.saturating_sub(1);
        }

        Ok(())
    }

    /// Get current bound socket count
    ///
    /// Used to determine when to enable/disable GIC interrupt.
    pub fn bound_count(&self) -> usize {
        self.bound_count
    }

    /// Lookup socket by key
    ///
    /// Used by packet router to find which socket(s) should receive a packet.
    ///
    /// # Arguments
    /// * `key` - Socket key to search for
    ///
    /// # Returns
    /// Socket ID if found, None otherwise.
    pub fn lookup(&self, key: &SocketKey) -> Option<usize> {
        // Try exact match first
        if let Some(&socket_id) = self.bindings.get(key) {
            return Some(socket_id);
        }

        // Try wildcard IP match (for AF_INET only)
        if key.local_ip.is_some() {
            let wildcard_key = SocketKey {
                protocol: key.protocol,
                local_ip: None,
                local_port: key.local_port,
            };
            if let Some(&socket_id) = self.bindings.get(&wildcard_key) {
                return Some(socket_id);
            }
        }

        None
    }

    /// Get socket by ID
    pub fn get(&self, sock: Socket) -> Option<&SocketImpl> {
        self.sockets.get(sock.0).and_then(|s| s.as_ref())
    }

    /// Get mutable socket by ID
    pub fn get_mut(&mut self, sock: Socket) -> Option<&mut SocketImpl> {
        self.sockets.get_mut(sock.0).and_then(|s| s.as_mut())
    }

    /// Allocate an ephemeral port
    fn alloc_ephemeral_port(&mut self) -> Result<u16, SocketError> {
        let start_port = self.next_ephemeral;

        loop {
            let port = self.next_ephemeral;

            // Advance for next allocation (with wrapping arithmetic to handle u16::MAX)
            self.next_ephemeral = if port == Self::EPHEMERAL_MAX {
                Self::EPHEMERAL_MIN
            } else {
                port + 1
            };

            // Check if port is in use
            let in_use = self.bindings.keys().any(|key| key.local_port == Some(port));

            if !in_use {
                return Ok(port);
            }

            // Wrapped around - no free ports
            if self.next_ephemeral == start_port {
                return Err(SocketError::AddressInUse);
            }
        }
    }

    /// Get socket table statistics
    ///
    /// Returns (total_allocated, bound_count, per_socket_queue_depths)
    pub fn stats(&self) -> (usize, usize, alloc::vec::Vec<(usize, usize)>) {
        let total = self.sockets.iter().filter(|s| s.is_some()).count();
        let bound = self.bound_count;

        // Collect per-socket queue depths for bound sockets
        let mut queue_depths = alloc::vec::Vec::new();
        for (idx, sock_opt) in self.sockets.iter().enumerate() {
            if let Some(sock) = sock_opt {
                if sock.state == SocketState::Bound {
                    queue_depths.push((idx, sock.rx_queue.len()));
                }
            }
        }

        (total, bound, queue_depths)
    }
}

/// Global socket table
pub static SOCKET_TABLE: Mutex<SocketTable> = Mutex::new(SocketTable {
    sockets: Vec::new(),
    bindings: BTreeMap::new(),
    next_ephemeral: SocketTable::EPHEMERAL_MIN,
    bound_count: 0,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_socket_create() {
        let mut table = SocketTable::new();

        let sock = table
            .create(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)
            .unwrap();
        assert_eq!(sock.0, 0);

        let socket = table.get(sock).unwrap();
        assert_eq!(socket.family, AddressFamily::Packet);
        assert_eq!(socket.socket_type, SocketType::Raw);
        assert_eq!(socket.protocol, Protocol::Arp);
        assert_eq!(socket.state, SocketState::Created);
    }

    #[test_case]
    fn test_socket_bind() {
        let mut table = SocketTable::new();

        let sock = table
            .create(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)
            .unwrap();

        let addr = SocketAddr::packet(0x0806);
        table.bind(sock, addr).unwrap();

        let socket = table.get(sock).unwrap();
        assert_eq!(socket.state, SocketState::Bound);
        assert!(socket.bind_addr.is_some());
    }

    #[test_case]
    fn test_socket_bind_duplicate() {
        let mut table = SocketTable::new();

        let sock1 = table
            .create(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)
            .unwrap();
        let sock2 = table
            .create(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)
            .unwrap();

        let addr = SocketAddr::packet(0x0806);
        table.bind(sock1, addr.clone()).unwrap();

        // Second bind to same address should fail
        assert_eq!(table.bind(sock2, addr), Err(SocketError::AddressInUse));
    }

    #[test_case]
    fn test_socket_lookup() {
        let mut table = SocketTable::new();

        let sock = table
            .create(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)
            .unwrap();

        let addr = SocketAddr::packet(0x0806);
        table.bind(sock, addr).unwrap();

        let key = SocketKey::packet(Protocol::Arp);
        let found = table.lookup(&key).unwrap();
        assert_eq!(found, sock.0);
    }

    #[test_case]
    fn test_socket_close() {
        let mut table = SocketTable::new();

        let sock = table
            .create(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)
            .unwrap();

        let addr = SocketAddr::packet(0x0806);
        table.bind(sock, addr.clone()).unwrap();

        table.close(sock).unwrap();

        // Socket should be freed
        assert!(table.get(sock).is_none());

        // Binding should be removed
        let key = SocketKey::packet(Protocol::Arp);
        assert!(table.lookup(&key).is_none());

        // Should be able to bind to same address again
        let sock2 = table
            .create(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)
            .unwrap();
        assert!(table.bind(sock2, addr).is_ok());
    }
}

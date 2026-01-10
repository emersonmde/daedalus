//! Socket Types and Constants
//!
//! Core type definitions for the socket API, inspired by POSIX sockets but adapted
//! for kernel-space use in a no_std environment.

use crate::drivers::net::netdev::NetworkError;
use crate::net::ethernet::MacAddress;
use core::fmt;

/// Socket address family (AF_* constants in POSIX)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressFamily {
    /// Raw Ethernet frames (Layer 2)
    ///
    /// Used for protocols that operate directly on Ethernet:
    /// - ARP (Address Resolution Protocol)
    /// - Custom Ethernet protocols
    Packet,

    /// Internet Protocol (Layer 3+)
    ///
    /// Used for IP-based protocols:
    /// - ICMP (ping)
    /// - UDP (DHCP, DNS)
    /// - TCP (HTTP, SSH)
    Inet,
}

/// Socket type (SOCK_* constants in POSIX)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketType {
    /// Raw socket - full protocol control
    ///
    /// Application builds complete protocol headers.
    /// For AF_PACKET: Build complete Ethernet frame
    /// For AF_INET: Build complete IP header + payload
    Raw,

    /// Datagram socket - connectionless
    ///
    /// Used for UDP: kernel handles IP/UDP headers
    Dgram,

    /// Stream socket - connection-oriented (future, for TCP)
    ///
    /// Reliable, ordered byte stream
    Stream,
}

/// Protocol types for socket creation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Protocol {
    /// No protocol (protocol-agnostic raw socket)
    None,

    /// ARP protocol (EtherType 0x0806)
    Arp,

    /// IPv4 protocol (EtherType 0x0800)
    Ipv4,

    /// ICMP protocol (IP protocol 1)
    Icmp,

    /// UDP protocol (IP protocol 17)
    Udp,

    /// TCP protocol (IP protocol 6)
    Tcp,
}

impl Protocol {
    /// Get EtherType for this protocol (if applicable)
    pub fn ethertype(&self) -> Option<u16> {
        match self {
            Protocol::Arp => Some(crate::net::ethernet::ETHERTYPE_ARP),
            Protocol::Ipv4 | Protocol::Icmp | Protocol::Udp | Protocol::Tcp => {
                Some(crate::net::ethernet::ETHERTYPE_IPV4)
            }
            Protocol::None => None,
        }
    }

    /// Get IP protocol number (if applicable)
    pub fn ip_proto(&self) -> Option<u8> {
        match self {
            Protocol::Icmp => Some(1),
            Protocol::Udp => Some(17),
            Protocol::Tcp => Some(6),
            _ => None,
        }
    }
}

/// Socket address for binding or connecting
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketAddr {
    /// Raw Ethernet address (AF_PACKET)
    ///
    /// Binds to specific EtherType, optionally filtering by interface MAC.
    Packet {
        /// Interface MAC address filter (None = any interface)
        interface: Option<MacAddress>,

        /// EtherType to filter (0 = all EtherTypes)
        protocol: u16,
    },

    /// Internet address (AF_INET)
    ///
    /// Binds to IP address and port.
    Inet {
        /// IPv4 address ([0,0,0,0] = INADDR_ANY, wildcard)
        ip: [u8; 4],

        /// Port number (0 for raw IP or ICMP)
        port: u16,
    },
}

impl SocketAddr {
    /// Create AF_PACKET address for specific EtherType
    pub fn packet(protocol: u16) -> Self {
        SocketAddr::Packet {
            interface: None,
            protocol,
        }
    }

    /// Create AF_INET address with wildcard IP (0.0.0.0)
    pub fn inet_any(port: u16) -> Self {
        SocketAddr::Inet {
            ip: [0, 0, 0, 0],
            port,
        }
    }

    /// Create AF_INET address with specific IP and port
    pub fn inet(ip: [u8; 4], port: u16) -> Self {
        SocketAddr::Inet { ip, port }
    }
}

impl fmt::Display for SocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocketAddr::Packet {
                interface,
                protocol,
            } => match interface {
                Some(mac) => write!(f, "{}:0x{:04X}", mac, protocol),
                None => write!(f, "*:0x{:04X}", protocol),
            },
            SocketAddr::Inet { ip, port } => {
                write!(f, "{}.{}.{}.{}:{}", ip[0], ip[1], ip[2], ip[3], port)
            }
        }
    }
}

/// Socket handle (opaque identifier)
///
/// Internally this is an index into the socket table.
/// Users should not rely on the internal representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Socket(pub(super) usize);

impl Socket {
    /// Create Socket from socket ID (internal use only)
    ///
    /// # Safety
    /// Caller must ensure the socket ID is valid in the global socket table.
    pub(crate) const fn from_id(id: usize) -> Self {
        Self(id)
    }
}

/// Socket options for send/recv operations
#[derive(Debug, Clone, Copy)]
pub struct SocketOptions {
    /// Non-blocking mode
    ///
    /// If true, recv returns WouldBlock immediately if no data available.
    /// If false, recv waits (spins) until data arrives or timeout.
    pub non_blocking: bool,

    /// Timeout in milliseconds (None = infinite)
    ///
    /// Only applies when non_blocking = false.
    pub timeout_ms: Option<u32>,
}

impl Default for SocketOptions {
    fn default() -> Self {
        Self {
            non_blocking: false,
            timeout_ms: None,
        }
    }
}

impl SocketOptions {
    /// Create blocking options (wait forever)
    pub fn blocking() -> Self {
        Self {
            non_blocking: false,
            timeout_ms: None,
        }
    }

    /// Create non-blocking options (return immediately)
    pub fn non_blocking() -> Self {
        Self {
            non_blocking: true,
            timeout_ms: None,
        }
    }

    /// Create blocking options with timeout
    pub fn with_timeout(timeout_ms: u32) -> Self {
        Self {
            non_blocking: false,
            timeout_ms: Some(timeout_ms),
        }
    }
}

/// Received packet metadata
///
/// Returned by recvfrom() to provide both data and source address.
#[derive(Debug, Clone, PartialEq)]
pub struct RecvFrom {
    /// Source address
    pub addr: SocketAddr,

    /// Received data (owned)
    pub data: alloc::vec::Vec<u8>,
}

/// Socket errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketError {
    /// Invalid address family for this operation
    InvalidAddressFamily,

    /// Invalid socket type for this operation
    InvalidSocketType,

    /// Protocol not supported
    ProtocolNotSupported,

    /// Operation would block (non-blocking mode)
    WouldBlock,

    /// Buffer too small for received data
    BufferTooSmall,

    /// Socket not bound to address
    NotBound,

    /// Address already in use (another socket bound to it)
    AddressInUse,

    /// Network device not available
    NetworkUnavailable,

    /// Underlying network error
    NetworkError(NetworkError),

    /// Invalid argument
    InvalidArgument,

    /// Invalid socket handle
    InvalidSocket,

    /// Connection refused (future, for TCP)
    ConnectionRefused,

    /// Connection reset (future, for TCP)
    ConnectionReset,

    /// Timeout waiting for operation
    Timeout,

    /// Out of memory (heap allocation failed)
    OutOfMemory,

    /// Too many sockets open
    TooManySockets,

    /// Socket already connected (future, for TCP)
    AlreadyConnected,

    /// Socket not connected (future, for TCP)
    NotConnected,
}

impl From<NetworkError> for SocketError {
    fn from(e: NetworkError) -> Self {
        SocketError::NetworkError(e)
    }
}

impl fmt::Display for SocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocketError::InvalidAddressFamily => write!(f, "Invalid address family"),
            SocketError::InvalidSocketType => write!(f, "Invalid socket type"),
            SocketError::ProtocolNotSupported => write!(f, "Protocol not supported"),
            SocketError::WouldBlock => write!(f, "Operation would block"),
            SocketError::BufferTooSmall => write!(f, "Buffer too small"),
            SocketError::NotBound => write!(f, "Socket not bound"),
            SocketError::AddressInUse => write!(f, "Address already in use"),
            SocketError::NetworkUnavailable => write!(f, "Network unavailable"),
            SocketError::NetworkError(e) => write!(f, "Network error: {}", e),
            SocketError::InvalidArgument => write!(f, "Invalid argument"),
            SocketError::InvalidSocket => write!(f, "Invalid socket"),
            SocketError::ConnectionRefused => write!(f, "Connection refused"),
            SocketError::ConnectionReset => write!(f, "Connection reset"),
            SocketError::Timeout => write!(f, "Timeout"),
            SocketError::OutOfMemory => write!(f, "Out of memory"),
            SocketError::TooManySockets => write!(f, "Too many sockets"),
            SocketError::AlreadyConnected => write!(f, "Already connected"),
            SocketError::NotConnected => write!(f, "Not connected"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test_case]
    fn test_protocol_ethertype() {
        assert_eq!(Protocol::Arp.ethertype(), Some(0x0806));
        assert_eq!(Protocol::Ipv4.ethertype(), Some(0x0800));
        assert_eq!(Protocol::Icmp.ethertype(), Some(0x0800));
        assert_eq!(Protocol::Udp.ethertype(), Some(0x0800));
        assert_eq!(Protocol::Tcp.ethertype(), Some(0x0800));
        assert_eq!(Protocol::None.ethertype(), None);
    }

    #[test_case]
    fn test_protocol_ip_proto() {
        assert_eq!(Protocol::Icmp.ip_proto(), Some(1));
        assert_eq!(Protocol::Udp.ip_proto(), Some(17));
        assert_eq!(Protocol::Tcp.ip_proto(), Some(6));
        assert_eq!(Protocol::Arp.ip_proto(), None);
    }

    #[test_case]
    fn test_socket_addr_display() {
        let addr = SocketAddr::packet(0x0806);
        assert_eq!(format!("{}", addr), "*:0x0806");

        let addr = SocketAddr::inet([192, 168, 1, 100], 8080);
        assert_eq!(format!("{}", addr), "192.168.1.100:8080");
    }

    #[test_case]
    fn test_socket_options() {
        let opts = SocketOptions::default();
        assert!(!opts.non_blocking);
        assert_eq!(opts.timeout_ms, None);

        let opts = SocketOptions::non_blocking();
        assert!(opts.non_blocking);

        let opts = SocketOptions::with_timeout(1000);
        assert!(!opts.non_blocking);
        assert_eq!(opts.timeout_ms, Some(1000));
    }
}

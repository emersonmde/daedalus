//! Network protocol implementations
//!
//! This module provides network protocol support for DaedalusOS, starting with
//! Ethernet frame handling and ARP (Address Resolution Protocol).

pub mod arp;
pub mod arp_diag;
pub mod ethernet;
pub mod http;
pub mod protocol;
pub mod protocols;
pub mod router;
pub mod skbuff;
pub mod socket;

// Re-export commonly used types
pub use arp::{ArpOperation, ArpPacket};
pub use arp_diag::run_arp_probe_diagnostic;
pub use ethernet::{ETHERTYPE_ARP, ETHERTYPE_IPV4, EthernetFrame, MacAddress};

// Re-export socket API
pub use socket::{
    AddressFamily, Protocol, RecvFrom, Socket, SocketAddr, SocketError, SocketOptions, SocketType,
    bind, close, recvfrom, sendto, socket,
};

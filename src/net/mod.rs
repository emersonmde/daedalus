//! Network protocol implementations
//!
//! This module provides network protocol support for DaedalusOS, starting with
//! Ethernet frame handling and ARP (Address Resolution Protocol).

pub mod arp;
pub mod ethernet;

// Re-export commonly used types
pub use arp::{ArpOperation, ArpPacket};
pub use ethernet::{ETHERTYPE_ARP, ETHERTYPE_IPV4, EthernetFrame, MacAddress};

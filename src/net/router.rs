//! Packet Router - Demultiplexing received frames to sockets
//!
//! This module is called from the GENET interrupt handler to route incoming
//! Ethernet frames to appropriate sockets based on protocol and address matching.
//!
//! ## Routing Strategy
//!
//! For Milestone #14 (AF_PACKET sockets only):
//! - Parse Ethernet header
//! - Match by EtherType (0x0806 for ARP, 0x0800 for IPv4, etc.)
//! - Deliver to all sockets bound to that EtherType
//!
//! Future milestones will add IP-layer routing (protocol + port matching).

use crate::net::ethernet::{ETHERTYPE_ARP, ETHERTYPE_IPV4, EthernetFrame};
use crate::net::packet_pool::PACKET_POOL;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// Statistics for packet routing
pub struct RouterStats {
    /// Total packets routed
    pub packets_routed: AtomicUsize,

    /// Packets dropped (parse error or no matching socket)
    pub packets_dropped: AtomicUsize,

    /// Packets dropped due to pool exhaustion
    pub pool_exhausted: AtomicUsize,
}

impl RouterStats {
    pub const fn new() -> Self {
        Self {
            packets_routed: AtomicUsize::new(0),
            packets_dropped: AtomicUsize::new(0),
            pool_exhausted: AtomicUsize::new(0),
        }
    }
}

/// Global router statistics
pub static ROUTER_STATS: RouterStats = RouterStats::new();

/// Route a received packet to appropriate socket(s)
///
/// Called from GENET interrupt handler for each received frame.
///
/// # Arguments
/// * `frame_data` - Raw Ethernet frame (including header)
///
/// # Returns
/// * `true` if packet was successfully routed to at least one socket
/// * `false` if packet was dropped (parse error, no matching socket, or pool full)
///
/// # Safety
/// Caller must ensure `frame_data` has static lifetime (points into GENET DMA buffer).
pub unsafe fn route_packet(frame_data: &'static [u8]) -> bool {
    // Parse Ethernet header
    let eth_frame = match EthernetFrame::parse(frame_data) {
        Some(frame) => frame,
        None => {
            // Malformed frame - drop silently
            ROUTER_STATS.packets_dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
    };

    // Route based on EtherType
    let routed = match eth_frame.ethertype {
        ETHERTYPE_ARP => route_arp(frame_data, &eth_frame),
        ETHERTYPE_IPV4 => route_ipv4(frame_data, &eth_frame),
        _ => {
            // Unknown EtherType - check for raw sockets
            route_raw(frame_data, &eth_frame)
        }
    };

    // Debug: Log first 20 dropped packets to see what we're filtering
    if !routed {
        static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
        let count = DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        if count < 20 {
            crate::println!(
                "[ROUTER] Drop #{}: EtherType 0x{:04X}, src={}, dst={}",
                count + 1,
                eth_frame.ethertype,
                eth_frame.src_mac,
                eth_frame.dest_mac
            );
        }
        ROUTER_STATS.packets_dropped.fetch_add(1, Ordering::Relaxed);
    } else {
        ROUTER_STATS.packets_routed.fetch_add(1, Ordering::Relaxed);
    }

    routed
}

/// Route ARP packet to bound sockets
///
/// # Arguments
/// * `frame_data` - Complete Ethernet frame
/// * `eth_frame` - Parsed Ethernet header
///
/// # Returns
/// `true` if delivered to at least one socket, `false` otherwise.
fn route_arp(frame_data: &'static [u8], _eth_frame: &EthernetFrame) -> bool {
    use crate::net::socket::{PacketRef, SOCKET_TABLE, SocketKey};

    // Allocate packet from pool
    // SAFETY: frame_data has static lifetime (from GENET DMA buffer)
    let buffer_id = unsafe {
        match PACKET_POOL.alloc(frame_data) {
            Some(id) => id,
            None => {
                ROUTER_STATS.pool_exhausted.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }
    };

    // Create packet reference
    let pkt_ref = PacketRef {
        buffer_id,
        offset: 0,
        length: frame_data.len(),
        timestamp: crate::drivers::timer::SystemTimer::timestamp_us(),
    };

    // Find socket bound to ARP protocol
    let key = SocketKey::packet(crate::net::socket::Protocol::Arp);

    let delivered = {
        let table = SOCKET_TABLE.lock();

        if let Some(socket_id) = table.lookup(&key) {
            // Use socket_id directly instead of trying to construct Socket
            if let Some(socket) = table.sockets.get(socket_id).and_then(|s| s.as_ref()) {
                // Enqueue packet to socket's RX queue
                if socket.rx_queue.enqueue(pkt_ref).is_ok() {
                    true
                } else {
                    // Socket queue full - packet dropped
                    false
                }
            } else {
                false
            }
        } else {
            // No socket bound to ARP
            false
        }
    };

    // If not delivered, free the packet buffer
    if !delivered {
        PACKET_POOL.free(buffer_id);
    }

    delivered
}

/// Route IPv4 packet to bound sockets (future)
///
/// For Milestone #14, this is a stub. Milestone #15 will add IP header parsing
/// and routing by protocol (ICMP, UDP, TCP) + port.
fn route_ipv4(_frame_data: &'static [u8], _eth_frame: &EthernetFrame) -> bool {
    // Future: Parse IP header, route by protocol + port
    // For now, drop (no IPv4 support yet)
    false
}

/// Route to raw sockets (future)
///
/// Raw sockets can bind to specific EtherTypes or receive all frames.
fn route_raw(_frame_data: &'static [u8], _eth_frame: &EthernetFrame) -> bool {
    // Future: Check for sockets bound to this EtherType
    // For now, drop
    false
}

/// Get router statistics
pub fn stats() -> (usize, usize, usize) {
    (
        ROUTER_STATS.packets_routed.load(Ordering::Relaxed),
        ROUTER_STATS.packets_dropped.load(Ordering::Relaxed),
        ROUTER_STATS.pool_exhausted.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::ethernet::MacAddress;
    extern crate alloc;
    use alloc::vec;

    #[test_case]
    fn test_route_malformed_frame() {
        // Too short to be valid Ethernet frame
        let buffer = [0u8; 10];

        // SAFETY: Transmuting to static lifetime for test purposes
        let frame_data: &'static [u8] = unsafe { core::mem::transmute(&buffer[..]) };
        let routed = unsafe { route_packet(frame_data) };

        // Should fail (malformed)
        assert!(!routed);
    }
}

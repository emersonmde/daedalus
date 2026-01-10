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

use crate::net::ethernet::{ETHERTYPE_ARP, EthernetFrame};
use crate::net::protocol::PROTOCOL_REGISTRY;
use crate::net::skbuff::SkBuff;
use crate::net::socket::Protocol;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Maximum debug log entries for unknown EtherTypes before silencing
/// Set to 0 in release builds (no logging in interrupt context)
#[cfg(debug_assertions)]
const DEBUG_UNKNOWN_ETHERTYPE_LIMIT: u32 = 10;

/// Maximum debug log entries for handler failures before silencing
#[cfg(debug_assertions)]
const DEBUG_HANDLER_FAILURE_LIMIT: u32 = 5;

/// Statistics for packet routing
pub struct RouterStats {
    /// Total packets routed
    pub packets_routed: AtomicUsize,

    /// Packets dropped (parse error or no matching socket)
    pub packets_dropped: AtomicUsize,

    /// Packets dropped due to allocation failure
    pub alloc_failed: AtomicUsize,
}

impl RouterStats {
    pub const fn new() -> Self {
        Self {
            packets_routed: AtomicUsize::new(0),
            packets_dropped: AtomicUsize::new(0),
            alloc_failed: AtomicUsize::new(0),
        }
    }
}

/// Global router statistics
pub static ROUTER_STATS: RouterStats = RouterStats::new();

/// Route a received packet to appropriate socket(s)
///
/// Called from GENET interrupt handler for each received frame.
/// Uses protocol registry to dispatch to registered handlers.
///
/// # Arguments
/// * `frame_data` - Raw Ethernet frame (including header)
///
/// # Returns
/// * `true` if packet was successfully routed to at least one socket
/// * `false` if packet was dropped (parse error, no matching socket, or allocation failed)
pub fn route_packet(frame_data: &[u8]) -> bool {
    // Parse Ethernet header
    let eth_frame = match EthernetFrame::parse(frame_data) {
        Some(frame) => frame,
        None => {
            // Malformed frame - drop silently
            ROUTER_STATS.packets_dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
    };

    // Map EtherType to Protocol enum
    let protocol = match eth_frame.ethertype {
        ETHERTYPE_ARP => Protocol::Arp,
        // Future: Parse IP header to determine ICMP/UDP/TCP
        // ETHERTYPE_IPV4 => parse_ip_protocol(frame_data),
        _ => {
            // Unknown protocol - drop packet
            ROUTER_STATS.packets_dropped.fetch_add(1, Ordering::Relaxed);

            // Debug: Log first N dropped packets (only in debug builds)
            #[cfg(debug_assertions)]
            {
                use core::sync::atomic::AtomicU32;
                static DROP_COUNT: AtomicU32 = AtomicU32::new(0);
                let count = DROP_COUNT.fetch_add(1, Ordering::Relaxed);
                if count < DEBUG_UNKNOWN_ETHERTYPE_LIMIT {
                    crate::println!(
                        "[ROUTER] Drop #{}: EtherType 0x{:04X}, src={}, dst={}",
                        count + 1,
                        eth_frame.ethertype,
                        eth_frame.src_mac,
                        eth_frame.dest_mac
                    );
                }
            }
            return false;
        }
    };

    // Allocate sk_buff (copy from DMA to heap)
    let skb = match SkBuff::from_dma(frame_data) {
        Ok(skb) => skb,
        Err(_) => {
            // Heap allocation failed
            ROUTER_STATS.alloc_failed.fetch_add(1, Ordering::Relaxed);
            ROUTER_STATS.packets_dropped.fetch_add(1, Ordering::Relaxed);
            return false;
        }
    };

    // Dispatch to protocol handler
    let routed = {
        let registry = PROTOCOL_REGISTRY.lock();
        if let Some(handler) = registry.get(&protocol) {
            // Call protocol-specific receive handler
            let result = handler.receive(skb);
            result.is_ok()
        } else {
            // No handler registered for this protocol
            false
        }
    }; // Lock dropped before any println

    // Update statistics and log issues
    if routed {
        ROUTER_STATS.packets_routed.fetch_add(1, Ordering::Relaxed);
    } else {
        ROUTER_STATS.packets_dropped.fetch_add(1, Ordering::Relaxed);

        // Debug: Log dropped packets (only in debug builds)
        #[cfg(debug_assertions)]
        {
            use core::sync::atomic::AtomicU32;
            static DROP_DEBUG_COUNT: AtomicU32 = AtomicU32::new(0);
            let debug_count = DROP_DEBUG_COUNT.fetch_add(1, Ordering::Relaxed);
            if debug_count < DEBUG_HANDLER_FAILURE_LIMIT {
                crate::println!(
                    "[ROUTER] Drop #{}: Protocol {:?} routing failed",
                    debug_count + 1,
                    protocol
                );
            }
        }
    }

    routed
}

/// Get router statistics
pub fn stats() -> (usize, usize, usize) {
    (
        ROUTER_STATS.packets_routed.load(Ordering::Relaxed),
        ROUTER_STATS.packets_dropped.load(Ordering::Relaxed),
        ROUTER_STATS.alloc_failed.load(Ordering::Relaxed),
    )
}

/// Print comprehensive router and network stack statistics
///
/// Call this after arp-probe runs to verify the new architecture is working.
pub fn print_debug_stats() {
    use crate::drivers::genet::genet_interrupt_stats;
    use crate::net::protocol::registered_protocol_count;
    use crate::net::protocols::arp_rx_count;
    use crate::net::socket::socket_stats;

    let (routed, dropped, alloc_failed) = stats();
    let protocol_count = registered_protocol_count();
    let arp_packets = arp_rx_count();
    let (sock_total, sock_bound, sock_queues) = socket_stats();
    let (irq_total, irq_spurious, rx_errors) = genet_interrupt_stats();

    crate::println!("\n=== Network Stack Statistics ===");

    crate::println!("Router:");
    crate::println!("  Packets routed:       {}", routed);
    crate::println!("  Packets dropped:      {}", dropped);
    crate::println!("  sk_buff alloc failed: {}", alloc_failed);

    crate::println!();
    crate::println!("Protocols:");
    crate::println!("  Registered:           {} (ARP)", protocol_count);
    crate::println!("  ARP packets RX:       {}", arp_packets);

    crate::println!();
    crate::println!("Sockets:");
    crate::println!("  Total allocated:      {}", sock_total);
    crate::println!("  Bound sockets:        {}", sock_bound);
    if !sock_queues.is_empty() {
        for (sock_id, depth) in sock_queues {
            crate::println!("  Socket {} RX queue:    {} packets", sock_id, depth);
        }
    }

    crate::println!();
    crate::println!("GENET Interrupts:");
    crate::println!("  Total IRQs:           {}", irq_total);
    crate::println!("  RX errors:            {}", rx_errors);
    crate::println!("  Spurious (current):   {}", irq_spurious);

    // Warnings
    if alloc_failed > 0 {
        crate::println!();
        crate::println!("  [WARNING] Heap allocation failures detected!");
        crate::println!("            This indicates heap exhaustion - check memory usage");
    }

    if dropped > 0 && routed == 0 {
        crate::println!();
        crate::println!("  [WARNING] No packets routed but some dropped!");
        crate::println!("            Check: Is protocol registry initialized?");
        crate::println!("                   Is socket bound?");
    }

    crate::println!("=====================================\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_route_malformed_frame() {
        // Too short to be valid Ethernet frame
        let buffer = [0u8; 10];

        let routed = route_packet(&buffer);

        // Should fail (malformed)
        assert!(!routed);
    }
}

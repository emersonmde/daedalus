//! Socket API - Network socket abstraction for DaedalusOS
//!
//! This module provides a POSIX-inspired socket API for kernel-space networking.
//! It supports interrupt-driven packet reception with port-based routing.
//!
//! ## Supported Socket Types
//!
//! ### AF_PACKET (Raw Ethernet)
//! - **Protocol**: ARP
//! - **Use case**: arp-probe diagnostic
//! - **Example**: See arp_diag.rs migration
//!
//! ### AF_INET (Future - Milestone #15+)
//! - **Protocols**: ICMP, UDP, TCP
//! - **Use cases**: ping, DHCP, DNS, HTTP
//!
//! ## Architecture
//!
//! ```text
//! Application → socket() → bind() → sendto()/recvfrom() → close()
//!                                          ↕
//!                                   Socket Table
//!                                          ↕
//!                              Lock-free RX Queue (per socket)
//!                                          ↕
//!                                 Packet Router (interrupt)
//!                                          ↕
//!                                   GENET Driver
//! ```

pub mod queue;
pub mod table;
pub mod types;

pub use queue::SkBuffQueue;
pub use table::{SOCKET_TABLE, SocketImpl, SocketKey, SocketState, SocketTable};
pub use types::{
    AddressFamily, Protocol, RecvFrom, Socket, SocketAddr, SocketError, SocketOptions, SocketType,
};

use crate::drivers::genet::GENET;
use crate::drivers::netdev::NetworkDevice;
use crate::drivers::timer;
use crate::net::ethernet::EthernetFrame;

/// Create a new socket
///
/// # Arguments
/// * `family` - Address family (Packet or Inet)
/// * `socket_type` - Socket type (Raw, Dgram, Stream)
/// * `protocol` - Protocol (Arp, Udp, Tcp, etc.)
///
/// # Returns
/// Socket handle on success.
///
/// # Errors
/// - `ProtocolNotSupported` - Invalid family/type/protocol combination
/// - `TooManySockets` - Maximum number of sockets exceeded
///
/// # Example
/// ```ignore
/// let sock = socket(AddressFamily::Packet, SocketType::Raw, Protocol::Arp)?;
/// ```
pub fn socket(
    family: AddressFamily,
    socket_type: SocketType,
    protocol: Protocol,
) -> Result<Socket, SocketError> {
    SOCKET_TABLE.lock().create(family, socket_type, protocol)
}

/// Bind socket to local address
///
/// # Arguments
/// * `sock` - Socket handle
/// * `addr` - Address to bind to
///
/// # Returns
/// Ok(()) on success.
///
/// # Errors
/// - `InvalidSocket` - Socket handle is invalid
/// - `InvalidAddressFamily` - Address family doesn't match socket
/// - `AddressInUse` - Address already bound by another socket
/// - `InvalidArgument` - Socket already bound
///
/// # Example
/// ```ignore
/// let addr = SocketAddr::packet(ETHERTYPE_ARP);
/// bind(sock, addr)?;
/// ```
pub fn bind(sock: Socket, addr: SocketAddr) -> Result<(), SocketError> {
    // Step 1: Bind socket and drain any leftover packets
    // (This fixes race condition where packets arrive between close and interrupt disable)
    let enable_interrupt = {
        let mut table = SOCKET_TABLE.lock();

        table.bind(sock, addr)?;

        // CRITICAL: Drain any stale packets from previous runs BEFORE enabling interrupts
        // Without this, packets that arrived during socket close race condition accumulate
        if let Some(socket) = table.get_mut(sock) {
            for _skb in socket.rx_queue.drain() {
                // Drain silently
            }
        }

        // Check if we should enable interrupt (just bound first socket)
        table.bound_count() == 1
    }; // SOCKET_TABLE lock dropped here

    // Step 2: Drain GENET RX ring and enable GIC interrupt OUTSIDE the SOCKET_TABLE lock
    // This prevents deadlock if interrupt fires immediately and tries to route packets
    // Note: Skip in test mode - GENET hardware not available in QEMU
    #[cfg(not(test))]
    if enable_interrupt {
        // CRITICAL: Drain any packets that accumulated in GENET RX ring while interrupts were disabled
        // Without this, enabling interrupts causes immediate flood of stale packets
        {
            let mut genet = GENET.lock();
            genet.drain_rx_ring();
        } // GENET lock dropped

        // Now enable interrupt
        {
            use crate::drivers::gic::GIC;
            use crate::drivers::gic::irq::GENET_0;
            let gic = GIC.lock();
            gic.enable_interrupt(GENET_0);
        } // GIC lock dropped here
    }

    Ok(())
}

/// Send data to specific address
///
/// # Arguments
/// * `sock` - Socket handle
/// * `buf` - Data to send
/// * `addr` - Destination address
///
/// # Returns
/// Number of bytes sent (currently always all or error).
///
/// # Errors
/// - `InvalidSocket` - Socket handle is invalid
/// - `NotBound` - Socket not bound to address
/// - `NetworkError` - Underlying network device error
///
/// # Example
/// ```ignore
/// let dest = SocketAddr::Packet {
///     interface: Some(MacAddress::broadcast()),
///     protocol: ETHERTYPE_ARP,
/// };
/// sendto(sock, &frame_buffer, &dest)?;
/// ```
pub fn sendto(sock: Socket, buf: &[u8], _addr: &SocketAddr) -> Result<usize, SocketError> {
    let table = SOCKET_TABLE.lock();
    let socket = table.get(sock).ok_or(SocketError::InvalidSocket)?;

    // Check if bound
    if socket.state != SocketState::Bound {
        return Err(SocketError::NotBound);
    }

    // For AF_PACKET, buf should be complete Ethernet frame
    // (caller builds frame with EthernetFrame::new + write_to)
    match socket.family {
        AddressFamily::Packet => {
            drop(table); // Release lock before GENET access

            // Transmit via GENET
            // Uses interrupt-safe Mutex (disables IRQs while held)
            let mut genet = GENET.lock();
            genet.transmit(buf)?;

            Ok(buf.len())
        }
        AddressFamily::Inet => {
            // Future: Wrap in IP header + Ethernet frame
            // Requires ARP resolution, IP routing, etc.
            Err(SocketError::ProtocolNotSupported)
        }
    }
}

/// Receive data from socket
///
/// # Arguments
/// * `sock` - Socket handle
/// * `opts` - Receive options (blocking, timeout)
///
/// # Returns
/// Received packet with source address and data.
///
/// # Errors
/// - `InvalidSocket` - Socket handle is invalid
/// - `NotBound` - Socket not bound to address
/// - `WouldBlock` - No data available (non-blocking mode)
/// - `Timeout` - Timeout expired waiting for data
///
/// # Example
/// ```ignore
/// let opts = SocketOptions::with_timeout(1000);  // 1 second
/// match recvfrom(sock, &opts) {
///     Ok(packet) => { /* process packet.data */ }
///     Err(SocketError::Timeout) => { /* handle timeout */ }
///     Err(e) => { /* handle error */ }
/// }
/// ```
pub fn recvfrom(sock: Socket, opts: &SocketOptions) -> Result<RecvFrom, SocketError> {
    let start_time = timer::SystemTimer::timestamp_us();
    let timeout_us = opts.timeout_ms.map(|ms| ms as u64 * 1000);

    loop {
        // Try to dequeue packet
        let skb = {
            let table = SOCKET_TABLE.lock();
            let socket = table.get(sock).ok_or(SocketError::InvalidSocket)?;

            if socket.state != SocketState::Bound {
                return Err(SocketError::NotBound);
            }

            socket.rx_queue.dequeue()
        };

        // If packet available, return it
        if let Some(skb) = skb {
            // Get packet data from sk_buff (already contains full frame)
            let data = skb.data().to_vec();

            // Parse source address from packet
            // For AF_PACKET: Extract source MAC from Ethernet header
            let addr = match EthernetFrame::parse(&data) {
                Some(frame) => SocketAddr::Packet {
                    interface: Some(frame.src_mac),
                    protocol: frame.ethertype,
                },
                None => {
                    // Malformed packet - use dummy address
                    SocketAddr::Packet {
                        interface: None,
                        protocol: 0,
                    }
                }
            };

            // Arc<SkBuff> dropped here, refcount decremented
            return Ok(RecvFrom { addr, data });
        }

        // No packet available
        if opts.non_blocking {
            return Err(SocketError::WouldBlock);
        }

        // Check timeout
        if let Some(timeout_us) = timeout_us {
            let elapsed = timer::SystemTimer::timestamp_us() - start_time;
            if elapsed >= timeout_us {
                return Err(SocketError::Timeout);
            }
        }

        // Yield CPU briefly before retrying
        core::hint::spin_loop();
    }
}

/// Close socket and free resources
///
/// # Arguments
/// * `sock` - Socket handle
///
/// # Returns
/// Ok(()) on success.
///
/// # Errors
/// - `InvalidSocket` - Socket handle is invalid
///
/// # Example
/// ```ignore
/// close(sock)?;
/// ```
pub fn close(sock: Socket) -> Result<(), SocketError> {
    // Step 1: Close socket and check if this was the last bound socket
    // (drop lock BEFORE disabling interrupt to prevent deadlock)
    let disable_interrupt = {
        let mut table = SOCKET_TABLE.lock();
        table.close(sock)?;

        // Check if we should disable interrupt (closed last bound socket)
        table.bound_count() == 0
    }; // SOCKET_TABLE lock dropped here

    // Step 2: Disable GIC interrupt OUTSIDE the SOCKET_TABLE lock
    // This prevents deadlock if an interrupt tries to route packets while we're closing
    // Note: Skip in test mode - GENET hardware not available in QEMU
    #[cfg(not(test))]
    if disable_interrupt {
        use crate::drivers::gic::GIC;
        use crate::drivers::gic::irq::GENET_0;

        // Disable interrupt
        {
            let gic = GIC.lock();
            gic.disable_interrupt(GENET_0);
        } // GIC lock dropped here
    }

    Ok(())
}

/// Get socket table statistics
///
/// Returns (total_allocated, bound_count, per_socket_queue_depths)
/// where queue_depths is a Vec of (socket_id, queue_depth) tuples.
pub fn socket_stats() -> (usize, usize, alloc::vec::Vec<(usize, usize)>) {
    SOCKET_TABLE.lock().stats()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::ethernet::ETHERTYPE_ARP;

    #[test_case]
    fn test_socket_lifecycle() {
        // Create socket
        let sock = socket(AddressFamily::Packet, SocketType::Raw, Protocol::Arp).unwrap();

        // Bind socket
        let addr = SocketAddr::packet(ETHERTYPE_ARP);
        bind(sock, addr).unwrap();

        // Close socket
        close(sock).unwrap();

        // Socket should be invalid after close
        assert_eq!(close(sock), Err(SocketError::InvalidSocket));
    }

    #[test_case]
    fn test_socket_bind_duplicate() {
        let sock1 = socket(AddressFamily::Packet, SocketType::Raw, Protocol::Arp).unwrap();
        let sock2 = socket(AddressFamily::Packet, SocketType::Raw, Protocol::Arp).unwrap();

        let addr = SocketAddr::packet(ETHERTYPE_ARP);
        bind(sock1, addr.clone()).unwrap();

        // Second bind should fail
        assert_eq!(bind(sock2, addr), Err(SocketError::AddressInUse));

        // Cleanup
        close(sock1).unwrap();
        close(sock2).unwrap();
    }

    #[test_case]
    fn test_recvfrom_non_blocking_would_block() {
        let sock = socket(AddressFamily::Packet, SocketType::Raw, Protocol::Arp).unwrap();
        let addr = SocketAddr::packet(ETHERTYPE_ARP);
        bind(sock, addr).unwrap();

        let opts = SocketOptions::non_blocking();
        let result = recvfrom(sock, &opts);

        assert_eq!(result, Err(SocketError::WouldBlock));

        close(sock).unwrap();
    }
}

//! ARP Probe Diagnostic Tool
//!
//! Comprehensive diagnostics for testing Ethernet TX/RX functionality.
//! Sends an ARP request and monitors all hardware state to diagnose issues.
//!
//! **Milestone #14 Update**: Now uses interrupt-driven socket API instead of direct
//! hardware polling. Packets are delivered via GENET RX interrupts to socket queues.

use crate::drivers::genet::GENET;
use crate::drivers::netdev::NetworkDevice;
use crate::drivers::timer::SystemTimer;
use crate::net::arp::{ArpOperation, ArpPacket};
use crate::net::ethernet::{ETHERTYPE_ARP, EthernetFrame, MacAddress};
use crate::net::{AddressFamily, Protocol, SocketAddr, SocketError, SocketOptions, SocketType};
use crate::net::{bind, close, recvfrom, sendto, socket};
use crate::{print, println};
use alloc::format;

/// Run comprehensive ARP probe diagnostic
///
/// This function:
/// 1. Checks hardware state before transmission
/// 2. Sends an ARP request to 10.42.10.1 via socket API
/// 3. Monitors DMA rings and MIB counters
/// 4. Receives packets via interrupt-driven socket (with details for first few)
/// 5. Looks for ARP reply
///
/// Returns true if ARP reply received, false otherwise.
pub fn run_arp_probe_diagnostic() -> bool {
    // Check if GENET hardware is present
    {
        let genet = GENET.lock();
        if !genet.is_present() {
            println!("[ERROR] GENET hardware not detected!");
            println!("[INFO] This diagnostic requires real hardware (not available in QEMU)");
            return false;
        }
    }

    // Create and bind ARP socket
    let sock = match socket(AddressFamily::Packet, SocketType::Raw, Protocol::Arp) {
        Ok(s) => s,
        Err(e) => {
            println!("[ERROR] Failed to create socket: {}", e);
            return false;
        }
    };

    let bind_addr = SocketAddr::packet(ETHERTYPE_ARP);
    if let Err(e) = bind(sock, bind_addr) {
        println!("[ERROR] Failed to bind socket: {}", e);
        close(sock).ok();
        return false;
    }

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║          GENET ETHERNET TX/RX DIAGNOSTIC TEST                ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // =======================================================================
    // STEP 1: Hardware State Before TX
    // =======================================================================

    println!("┌─ Hardware State (Pre-TX) ────────────────────────────────────");

    let (our_mac, tx_prod_pre, tx_cons_pre, stats_before) = {
        let genet = GENET.lock();
        let mac = genet.mac_address();
        let (prod, cons) = genet.read_dma_indices();
        let stats = genet.read_stats();
        (mac, prod, cons, stats)
    };

    println!("│ MAC Address:    {}", our_mac);
    println!("│ TX PROD_INDEX:  {}", tx_prod_pre);
    println!("│ TX CONS_INDEX:  {}", tx_cons_pre);
    println!("│");
    println!("│ MIB Counters (Before TX):");
    println!("│   TX Packets:   {}", stats_before.tx_packets);
    println!("│   TX Bytes:     {}", stats_before.tx_bytes);
    println!("│   RX Packets:   {}", stats_before.rx_packets);
    println!("│   RX Bytes:     {}", stats_before.rx_bytes);
    println!("└──────────────────────────────────────────────────────────────");
    println!();

    // =======================================================================
    // STEP 2: Build and Send ARP Request
    // =======================================================================

    let our_ip = [10, 42, 10, 100];
    let target_ip = [10, 42, 10, 1];

    println!("┌─ ARP Request ────────────────────────────────────────────────");
    println!(
        "│ Our IP:         {}.{}.{}.{}",
        our_ip[0], our_ip[1], our_ip[2], our_ip[3]
    );
    println!(
        "│ Target IP:      {}.{}.{}.{}",
        target_ip[0], target_ip[1], target_ip[2], target_ip[3]
    );
    println!("│ Operation:      REQUEST (who-has)");
    println!("└──────────────────────────────────────────────────────────────");
    println!();

    // Build ARP request
    let arp_request = ArpPacket::request(our_mac, our_ip, target_ip);
    let mut arp_buffer = [0u8; 28];
    let arp_size = arp_request
        .write_to(&mut arp_buffer)
        .expect("ARP buffer size");

    // Build Ethernet frame
    let eth_frame = EthernetFrame::new(
        MacAddress::broadcast(),
        our_mac,
        ETHERTYPE_ARP,
        &arp_buffer[..arp_size],
    );

    let mut frame_buffer = [0u8; 64];
    let frame_size = eth_frame
        .write_to(&mut frame_buffer)
        .expect("Frame buffer size");
    let send_size = if frame_size < 64 { 64 } else { frame_size };

    println!("┌─ Transmission ───────────────────────────────────────────────");
    println!(
        "│ Frame Size:     {} bytes (padded to {} bytes)",
        frame_size, send_size
    );

    // Send the frame via socket API
    // For AF_PACKET, we send the complete Ethernet frame (caller builds it)
    let dest_addr = SocketAddr::Packet {
        interface: Some(MacAddress::broadcast()),
        protocol: ETHERTYPE_ARP,
    };

    match sendto(sock, &frame_buffer[..send_size], &dest_addr) {
        Ok(_) => println!("│ TX Result:      ✓ SUCCESS"),
        Err(e) => {
            println!("│ TX Result:      ✗ FAILED: {}", e);
            println!("└──────────────────────────────────────────────────────────────");
            close(sock).ok();
            return false;
        }
    }

    // Wait for hardware to process
    SystemTimer::delay_ms(10);

    // Check DMA indices and stats after TX
    let (tx_prod_post, tx_cons_post, stats_after_tx) = {
        let genet = GENET.lock();
        let (prod, cons) = genet.read_dma_indices();
        let stats = genet.read_stats();
        (prod, cons, stats)
    };

    println!("│ TX PROD (after): {}", tx_prod_post);
    println!("│ TX CONS (after): {}", tx_cons_post);

    if tx_cons_post > tx_cons_pre {
        println!("│ DMA Status:      ✓ Hardware processed descriptor (CONS advanced)");
    } else {
        println!("│ DMA Status:      ✗ CONS unchanged (hardware may not be working)");
    }

    println!("│");
    println!("│ MIB Counters (After TX):");
    println!(
        "│   TX Packets:    {} (delta: {})",
        stats_after_tx.tx_packets,
        stats_after_tx
            .tx_packets
            .saturating_sub(stats_before.tx_packets)
    );
    println!(
        "│   TX Bytes:      {} (delta: {})",
        stats_after_tx.tx_bytes,
        stats_after_tx
            .tx_bytes
            .saturating_sub(stats_before.tx_bytes)
    );

    if stats_after_tx.tx_packets > stats_before.tx_packets {
        println!("│ MAC Status:      ✓ MAC incremented TX counter");
    } else {
        println!(
            "│ MAC Status:      ✗ MAC did NOT increment counter (packet may not have been sent)"
        );
    }

    println!("└──────────────────────────────────────────────────────────────");
    println!();

    // =======================================================================
    // STEP 3: Receive Packets and Look for ARP Reply
    // =======================================================================

    println!("┌─ Reception (polling socket for 2 seconds) ───────────────────");
    println!("│");

    let mut arp_reply_received = false;
    let mut total_packets_received = 0;
    let mut watchdog_triggered = false;

    // Use non-blocking socket options for polling
    let recv_opts = SocketOptions::non_blocking();

    for iteration in 0..20 {
        // Drain all packets from socket queue
        let mut iteration_packet_count = 0;

        loop {
            // Try to receive packet from socket (non-blocking)
            let rx_packet = match recvfrom(sock, &recv_opts) {
                Ok(pkt) => pkt,
                Err(SocketError::WouldBlock) => break, // No more packets in queue
                Err(e) => {
                    println!("│ [ERROR] Socket recv failed: {}", e);
                    break;
                }
            };

            let rx_frame_data = &rx_packet.data;
            total_packets_received += 1;
            iteration_packet_count += 1;

            // Safety check
            if iteration_packet_count > 100 {
                println!(
                    "│ [WATCHDOG] Iteration {} processed 100+ packets! Breaking.",
                    iteration + 1
                );
                watchdog_triggered = true;
                break;
            }

            // Show details for first 3 packets only
            if total_packets_received <= 3 {
                println!(
                    "│ ┌─ Packet #{} ({} bytes) ─────────",
                    total_packets_received,
                    rx_frame_data.len()
                );

                // Show first 32 bytes of raw data
                print!("│ │ Raw: ");
                for (i, byte) in rx_frame_data.iter().take(32).enumerate() {
                    if i > 0 && i % 16 == 0 {
                        print!("\n│ │      ");
                    }
                    print!("{:02X} ", byte);
                }
                println!();

                // Parse Ethernet frame
                if let Some(rx_frame) = EthernetFrame::parse(rx_frame_data) {
                    println!("│ │ Dest MAC:  {}", rx_frame.dest_mac);
                    println!("│ │ Src MAC:   {}", rx_frame.src_mac);
                    println!("│ │ EtherType: 0x{:04X}", rx_frame.ethertype);

                    // Check if it's ARP
                    if rx_frame.ethertype == ETHERTYPE_ARP {
                        if let Some(arp_pkt) = ArpPacket::parse(rx_frame.payload) {
                            println!("│ │ ARP Op:    {:?}", arp_pkt.operation);
                            println!(
                                "│ │ Sender:    {} at {}",
                                format!(
                                    "{}.{}.{}.{}",
                                    arp_pkt.sender_ip[0],
                                    arp_pkt.sender_ip[1],
                                    arp_pkt.sender_ip[2],
                                    arp_pkt.sender_ip[3]
                                ),
                                arp_pkt.sender_mac
                            );

                            if arp_pkt.operation == ArpOperation::Reply {
                                println!("│ └─ ✓ ARP REPLY FOUND! ─────────────────────────────");
                                arp_reply_received = true;
                            } else {
                                println!("│ └───────────────────────────────────────────────────");
                            }
                        } else {
                            println!("│ │ (Failed to parse ARP packet)");
                            println!("│ └───────────────────────────────────────────────────");
                        }
                    } else {
                        println!("│ └───────────────────────────────────────────────────");
                    }
                } else {
                    println!("│ │ (Failed to parse Ethernet frame)");
                    println!("│ └───────────────────────────────────────────────────");
                }
            } else {
                // Just parse to check for ARP reply (don't print details for packets > 3)
                // Parse Ethernet frame
                if let Some(rx_frame) = EthernetFrame::parse(rx_frame_data) {
                    // Check if it's an ARP packet
                    if rx_frame.ethertype == ETHERTYPE_ARP {
                        // Parse ARP packet
                        if let Some(arp_pkt) = ArpPacket::parse(rx_frame.payload) {
                            // Check if it's a reply
                            if arp_pkt.operation == ArpOperation::Reply {
                                println!(
                                    "│ Packet #{}: ✓ ARP REPLY from {}.{}.{}.{} at {}",
                                    total_packets_received,
                                    arp_pkt.sender_ip[0],
                                    arp_pkt.sender_ip[1],
                                    arp_pkt.sender_ip[2],
                                    arp_pkt.sender_ip[3],
                                    arp_pkt.sender_mac
                                );
                                arp_reply_received = true;
                            }
                        }
                    }
                }
            }
            // Note: Socket API automatically frees packet buffer when rx_packet is dropped
        }

        // Show iteration summary if we got packets
        if iteration_packet_count > 3 {
            println!(
                "│ (Iteration {} processed {} additional packets)",
                iteration + 1,
                iteration_packet_count
            );
        }

        // If we found ARP reply, we can stop
        if arp_reply_received {
            break;
        }

        // Wait before next iteration
        SystemTimer::delay_ms(100);
    }

    println!("│");
    println!("│ Total Packets:  {}", total_packets_received);
    println!("└──────────────────────────────────────────────────────────────");
    println!();

    // Close socket
    close(sock).ok();

    // =======================================================================
    // STEP 4: Final Statistics
    // =======================================================================

    let stats_final = {
        let genet = GENET.lock();
        genet.read_stats()
    };

    println!("┌─ Final MIB Counters ─────────────────────────────────────────");
    println!(
        "│ TX Packets:     {} (sent {} this test)",
        stats_final.tx_packets,
        stats_final
            .tx_packets
            .saturating_sub(stats_before.tx_packets)
    );
    println!(
        "│ TX Bytes:       {} (sent {} bytes)",
        stats_final.tx_bytes,
        stats_final.tx_bytes.saturating_sub(stats_before.tx_bytes)
    );
    println!(
        "│ RX Packets:     {} (received {} this test)",
        stats_final.rx_packets,
        stats_final
            .rx_packets
            .saturating_sub(stats_before.rx_packets)
    );
    println!(
        "│ RX Bytes:       {} (received {} bytes)",
        stats_final.rx_bytes,
        stats_final.rx_bytes.saturating_sub(stats_before.rx_bytes)
    );
    println!("│ RX FCS Errors:  {}", stats_final.rx_fcs_errors);
    println!("│ RX Align Errors: {}", stats_final.rx_align_errors);
    println!("└──────────────────────────────────────────────────────────────");
    println!();

    // =======================================================================
    // STEP 5: Verdict
    // =======================================================================

    println!("╔═══════════════════════════════════════════════════════════════╗");
    if arp_reply_received {
        println!("║  ✓✓✓ TEST PASSED - ETHERNET TX/RX IS WORKING! ✓✓✓          ║");
        println!("║                                                               ║");
        println!("║  ARP reply received from router.                             ║");
        println!("║  Both transmission and reception are functional.             ║");
    } else if total_packets_received > 0 {
        println!("║  ⚠ PARTIAL SUCCESS - RX WORKING, NO ARP REPLY                ║");
        println!("║                                                               ║");
        println!(
            "║  Received {} packet(s) but no ARP reply from 10.42.10.1      ║",
            total_packets_received
        );
        println!("║  This may mean:                                              ║");
        println!("║  - Router is not at 10.42.10.1 (check network config)        ║");
        println!("║  - Firewall blocking ARP                                     ║");
        println!("║  - Wrong subnet (Pi not on 10.42.10.0/24)                    ║");
    } else if stats_final.tx_packets > stats_before.tx_packets {
        println!("║  ⚠ TX WORKING, RX NOT RECEIVING                              ║");
        println!("║                                                               ║");
        println!("║  TX MIB counter incremented (MAC sent the frame)             ║");
        println!("║  But no packets received.                                    ║");
        println!("║  Possible issues:                                            ║");
        println!("║  - RX DMA not configured correctly                           ║");
        println!("║  - MAC filtering too strict                                  ║");
        println!("║  - Cable unplugged or link down                              ║");
    } else {
        println!("║  ✗✗✗ TEST FAILED - TX NOT WORKING ✗✗✗                        ║");
        println!("║                                                               ║");
        println!("║  TX MIB counter did NOT increment.                           ║");
        println!("║  Hardware did not transmit the frame.                        ║");
        println!("║  Possible issues:                                            ║");
        println!("║  - UMAC not enabled (TX_EN/RX_EN not set)                    ║");
        println!("║  - PHY link is down                                          ║");
        println!("║  - DMA not configured correctly                              ║");
        println!("║  - UMAC_CMD or DMA registers misconfigured                   ║");
    }

    if watchdog_triggered {
        println!("║                                                               ║");
        println!("║  ⚠ WARNING: Watchdog triggered (packet storm detected)       ║");
        println!("║  Network may be experiencing high traffic or broadcast storm ║");
    }

    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    arp_reply_received
}

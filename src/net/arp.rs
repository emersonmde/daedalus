//! ARP (Address Resolution Protocol) implementation
//!
//! ARP is used to map IP addresses to MAC addresses on a local network.
//! This module provides structures for parsing and constructing ARP packets.
//!
//! Reference: RFC 826 - <https://www.rfc-editor.org/rfc/rfc826>

use super::ethernet::MacAddress;
use core::fmt;

/// ARP operation codes
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ArpOperation {
    /// ARP request (who has IP address X?)
    Request = 1,
    /// ARP reply (IP address X is at MAC address Y)
    Reply = 2,
}

impl ArpOperation {
    /// Convert from u16 in network byte order
    pub fn from_be(value: u16) -> Option<Self> {
        match value {
            1 => Some(ArpOperation::Request),
            2 => Some(ArpOperation::Reply),
            _ => None,
        }
    }

    /// Convert to u16 in network byte order
    pub fn to_be(self) -> u16 {
        self as u16
    }
}

/// ARP packet for Ethernet and IPv4
///
/// Total size: 28 bytes
/// - Hardware type: 2 bytes (1 = Ethernet)
/// - Protocol type: 2 bytes (0x0800 = IPv4)
/// - Hardware address length: 1 byte (6 for MAC)
/// - Protocol address length: 1 byte (4 for IPv4)
/// - Operation: 2 bytes (1 = request, 2 = reply)
/// - Sender hardware address: 6 bytes (MAC)
/// - Sender protocol address: 4 bytes (IPv4)
/// - Target hardware address: 6 bytes (MAC)
/// - Target protocol address: 4 bytes (IPv4)
#[derive(Copy, Clone, Debug)]
pub struct ArpPacket {
    pub hardware_type: u16,
    pub protocol_type: u16,
    pub hw_addr_len: u8,
    pub proto_addr_len: u8,
    pub operation: ArpOperation,
    pub sender_mac: MacAddress,
    pub sender_ip: [u8; 4],
    pub target_mac: MacAddress,
    pub target_ip: [u8; 4],
}

// Hardware type constants
/// Ethernet hardware type
pub const ARP_HARDWARE_ETHERNET: u16 = 1;

// Protocol type constants
/// IPv4 protocol type
pub const ARP_PROTOCOL_IPV4: u16 = 0x0800;

impl ArpPacket {
    /// Size of an ARP packet in bytes
    pub const SIZE: usize = 28;

    /// Create a new ARP packet for Ethernet/IPv4
    pub fn new(
        operation: ArpOperation,
        sender_mac: MacAddress,
        sender_ip: [u8; 4],
        target_mac: MacAddress,
        target_ip: [u8; 4],
    ) -> Self {
        Self {
            hardware_type: ARP_HARDWARE_ETHERNET,
            protocol_type: ARP_PROTOCOL_IPV4,
            hw_addr_len: 6,
            proto_addr_len: 4,
            operation,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        }
    }

    /// Create an ARP request
    ///
    /// Asks "Who has target_ip? Tell sender_ip (at sender_mac)"
    pub fn request(sender_mac: MacAddress, sender_ip: [u8; 4], target_ip: [u8; 4]) -> Self {
        Self::new(
            ArpOperation::Request,
            sender_mac,
            sender_ip,
            MacAddress::zero(), // Target MAC is unknown in request
            target_ip,
        )
    }

    /// Create an ARP reply
    ///
    /// Says "target_ip is at target_mac"
    pub fn reply(
        sender_mac: MacAddress,
        sender_ip: [u8; 4],
        target_mac: MacAddress,
        target_ip: [u8; 4],
    ) -> Self {
        Self::new(
            ArpOperation::Reply,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        )
    }

    /// Parse an ARP packet from raw bytes
    ///
    /// Returns None if the buffer is too short or contains invalid data.
    pub fn parse(buffer: &[u8]) -> Option<Self> {
        if buffer.len() < Self::SIZE {
            return None;
        }

        // Parse hardware type (bytes 0-1, big-endian)
        let hardware_type = u16::from_be_bytes([buffer[0], buffer[1]]);

        // Parse protocol type (bytes 2-3, big-endian)
        let protocol_type = u16::from_be_bytes([buffer[2], buffer[3]]);

        // Parse address lengths (bytes 4-5)
        let hw_addr_len = buffer[4];
        let proto_addr_len = buffer[5];

        // Verify this is Ethernet/IPv4
        if hardware_type != ARP_HARDWARE_ETHERNET
            || protocol_type != ARP_PROTOCOL_IPV4
            || hw_addr_len != 6
            || proto_addr_len != 4
        {
            return None;
        }

        // Parse operation (bytes 6-7, big-endian)
        let operation = ArpOperation::from_be(u16::from_be_bytes([buffer[6], buffer[7]]))?;

        // Parse sender hardware address (bytes 8-13)
        let mut sender_mac_bytes = [0u8; 6];
        sender_mac_bytes.copy_from_slice(&buffer[8..14]);
        let sender_mac = MacAddress(sender_mac_bytes);

        // Parse sender protocol address (bytes 14-17)
        let mut sender_ip = [0u8; 4];
        sender_ip.copy_from_slice(&buffer[14..18]);

        // Parse target hardware address (bytes 18-23)
        let mut target_mac_bytes = [0u8; 6];
        target_mac_bytes.copy_from_slice(&buffer[18..24]);
        let target_mac = MacAddress(target_mac_bytes);

        // Parse target protocol address (bytes 24-27)
        let mut target_ip = [0u8; 4];
        target_ip.copy_from_slice(&buffer[24..28]);

        Some(Self {
            hardware_type,
            protocol_type,
            hw_addr_len,
            proto_addr_len,
            operation,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        })
    }

    /// Write this ARP packet to a buffer
    ///
    /// Returns the number of bytes written, or None if the buffer is too small.
    pub fn write_to(&self, buffer: &mut [u8]) -> Option<usize> {
        if buffer.len() < Self::SIZE {
            return None;
        }

        // Write hardware type (big-endian)
        buffer[0..2].copy_from_slice(&self.hardware_type.to_be_bytes());

        // Write protocol type (big-endian)
        buffer[2..4].copy_from_slice(&self.protocol_type.to_be_bytes());

        // Write address lengths
        buffer[4] = self.hw_addr_len;
        buffer[5] = self.proto_addr_len;

        // Write operation (big-endian)
        buffer[6..8].copy_from_slice(&self.operation.to_be().to_be_bytes());

        // Write sender hardware address
        buffer[8..14].copy_from_slice(&self.sender_mac.0);

        // Write sender protocol address
        buffer[14..18].copy_from_slice(&self.sender_ip);

        // Write target hardware address
        buffer[18..24].copy_from_slice(&self.target_mac.0);

        // Write target protocol address
        buffer[24..28].copy_from_slice(&self.target_ip);

        Some(Self::SIZE)
    }
}

impl fmt::Display for ArpPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = match self.operation {
            ArpOperation::Request => "Request",
            ArpOperation::Reply => "Reply",
        };

        write!(
            f,
            "ARP {} - Who has {}.{}.{}.{}? Tell {}.{}.{}.{} ({})",
            op,
            self.target_ip[0],
            self.target_ip[1],
            self.target_ip[2],
            self.target_ip[3],
            self.sender_ip[0],
            self.sender_ip[1],
            self.sender_ip[2],
            self.sender_ip[3],
            self.sender_mac
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    use alloc::string::ToString;

    #[test_case]
    fn test_arp_operation_conversion() {
        assert_eq!(ArpOperation::from_be(1), Some(ArpOperation::Request));
        assert_eq!(ArpOperation::from_be(2), Some(ArpOperation::Reply));
        assert_eq!(ArpOperation::from_be(3), None);
        assert_eq!(ArpOperation::from_be(0), None);

        assert_eq!(ArpOperation::Request.to_be(), 1);
        assert_eq!(ArpOperation::Reply.to_be(), 2);
    }

    #[test_case]
    fn test_arp_request_creation() {
        let sender_mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        let sender_ip = [192, 168, 1, 100];
        let target_ip = [192, 168, 1, 1];

        let request = ArpPacket::request(sender_mac, sender_ip, target_ip);

        assert_eq!(request.hardware_type, ARP_HARDWARE_ETHERNET);
        assert_eq!(request.protocol_type, ARP_PROTOCOL_IPV4);
        assert_eq!(request.hw_addr_len, 6);
        assert_eq!(request.proto_addr_len, 4);
        assert_eq!(request.operation, ArpOperation::Request);
        assert_eq!(request.sender_mac, sender_mac);
        assert_eq!(request.sender_ip, sender_ip);
        assert_eq!(request.target_mac, MacAddress::zero());
        assert_eq!(request.target_ip, target_ip);
    }

    #[test_case]
    fn test_arp_reply_creation() {
        let sender_mac = MacAddress::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let sender_ip = [192, 168, 1, 1];
        let target_mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        let target_ip = [192, 168, 1, 100];

        let reply = ArpPacket::reply(sender_mac, sender_ip, target_mac, target_ip);

        assert_eq!(reply.operation, ArpOperation::Reply);
        assert_eq!(reply.sender_mac, sender_mac);
        assert_eq!(reply.sender_ip, sender_ip);
        assert_eq!(reply.target_mac, target_mac);
        assert_eq!(reply.target_ip, target_ip);
    }

    #[test_case]
    fn test_arp_packet_write() {
        let sender_mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        let sender_ip = [192, 168, 1, 100];
        let target_ip = [192, 168, 1, 1];

        let request = ArpPacket::request(sender_mac, sender_ip, target_ip);

        let mut buffer = [0u8; 64];
        let size = request.write_to(&mut buffer).unwrap();

        assert_eq!(size, ArpPacket::SIZE);

        // Verify hardware type (0x0001)
        assert_eq!(&buffer[0..2], &[0x00, 0x01]);

        // Verify protocol type (0x0800 for IPv4)
        assert_eq!(&buffer[2..4], &[0x08, 0x00]);

        // Verify address lengths
        assert_eq!(buffer[4], 6); // HW addr len
        assert_eq!(buffer[5], 4); // Proto addr len

        // Verify operation (0x0001 for request)
        assert_eq!(&buffer[6..8], &[0x00, 0x01]);

        // Verify sender MAC
        assert_eq!(&buffer[8..14], &[0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);

        // Verify sender IP
        assert_eq!(&buffer[14..18], &[192, 168, 1, 100]);

        // Verify target MAC (zeros for request)
        assert_eq!(&buffer[18..24], &[0x00; 6]);

        // Verify target IP
        assert_eq!(&buffer[24..28], &[192, 168, 1, 1]);
    }

    #[test_case]
    fn test_arp_packet_parse() {
        let mut buffer = [0u8; 28];

        // Hardware type: Ethernet (0x0001)
        buffer[0..2].copy_from_slice(&[0x00, 0x01]);

        // Protocol type: IPv4 (0x0800)
        buffer[2..4].copy_from_slice(&[0x08, 0x00]);

        // Address lengths
        buffer[4] = 6; // MAC
        buffer[5] = 4; // IPv4

        // Operation: Request (0x0001)
        buffer[6..8].copy_from_slice(&[0x00, 0x01]);

        // Sender MAC
        buffer[8..14].copy_from_slice(&[0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);

        // Sender IP
        buffer[14..18].copy_from_slice(&[192, 168, 1, 100]);

        // Target MAC (zeros)
        buffer[18..24].copy_from_slice(&[0x00; 6]);

        // Target IP
        buffer[24..28].copy_from_slice(&[192, 168, 1, 1]);

        let packet = ArpPacket::parse(&buffer).unwrap();

        assert_eq!(packet.hardware_type, ARP_HARDWARE_ETHERNET);
        assert_eq!(packet.protocol_type, ARP_PROTOCOL_IPV4);
        assert_eq!(packet.operation, ArpOperation::Request);
        assert_eq!(
            packet.sender_mac,
            MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56])
        );
        assert_eq!(packet.sender_ip, [192, 168, 1, 100]);
        assert_eq!(packet.target_mac, MacAddress::zero());
        assert_eq!(packet.target_ip, [192, 168, 1, 1]);
    }

    #[test_case]
    fn test_arp_packet_parse_too_short() {
        let buffer = [0u8; 10]; // Too short
        assert!(ArpPacket::parse(&buffer).is_none());
    }

    #[test_case]
    fn test_arp_packet_parse_wrong_hardware_type() {
        let mut buffer = [0u8; 28];
        buffer[0..2].copy_from_slice(&[0x00, 0x06]); // Not Ethernet
        buffer[2..4].copy_from_slice(&[0x08, 0x00]); // IPv4
        buffer[4] = 6;
        buffer[5] = 4;
        buffer[6..8].copy_from_slice(&[0x00, 0x01]); // Request

        assert!(ArpPacket::parse(&buffer).is_none());
    }

    #[test_case]
    fn test_arp_packet_parse_wrong_protocol() {
        let mut buffer = [0u8; 28];
        buffer[0..2].copy_from_slice(&[0x00, 0x01]); // Ethernet
        buffer[2..4].copy_from_slice(&[0x86, 0xDD]); // IPv6
        buffer[4] = 6;
        buffer[5] = 4;
        buffer[6..8].copy_from_slice(&[0x00, 0x01]); // Request

        assert!(ArpPacket::parse(&buffer).is_none());
    }

    #[test_case]
    fn test_arp_packet_parse_invalid_operation() {
        let mut buffer = [0u8; 28];
        buffer[0..2].copy_from_slice(&[0x00, 0x01]); // Ethernet
        buffer[2..4].copy_from_slice(&[0x08, 0x00]); // IPv4
        buffer[4] = 6;
        buffer[5] = 4;
        buffer[6..8].copy_from_slice(&[0x00, 0x99]); // Invalid operation

        assert!(ArpPacket::parse(&buffer).is_none());
    }

    #[test_case]
    fn test_arp_packet_roundtrip() {
        let sender_mac = MacAddress::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let sender_ip = [10, 0, 0, 1];
        let target_mac = MacAddress::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        let target_ip = [10, 0, 0, 2];

        let reply = ArpPacket::reply(sender_mac, sender_ip, target_mac, target_ip);

        let mut buffer = [0u8; 64];
        let size = reply.write_to(&mut buffer).unwrap();
        assert_eq!(size, ArpPacket::SIZE);

        let parsed = ArpPacket::parse(&buffer[..size]).unwrap();

        assert_eq!(parsed.operation, ArpOperation::Reply);
        assert_eq!(parsed.sender_mac, sender_mac);
        assert_eq!(parsed.sender_ip, sender_ip);
        assert_eq!(parsed.target_mac, target_mac);
        assert_eq!(parsed.target_ip, target_ip);
    }

    #[test_case]
    fn test_arp_packet_display() {
        let sender_mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        let sender_ip = [192, 168, 1, 100];
        let target_ip = [192, 168, 1, 1];

        let request = ArpPacket::request(sender_mac, sender_ip, target_ip);
        let display = request.to_string();

        assert!(display.contains("Request"));
        assert!(display.contains("192.168.1.1"));
        assert!(display.contains("192.168.1.100"));
    }

    #[test_case]
    fn test_arp_write_buffer_too_small() {
        let sender_mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        let sender_ip = [192, 168, 1, 100];
        let target_ip = [192, 168, 1, 1];

        let request = ArpPacket::request(sender_mac, sender_ip, target_ip);

        let mut buffer = [0u8; 10]; // Too small
        assert!(request.write_to(&mut buffer).is_none());
    }
}

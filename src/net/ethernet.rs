//! Ethernet frame handling
//!
//! Provides data structures and utilities for working with Ethernet II frames.
//! Includes MAC address representation, frame parsing, and construction.

use core::fmt;
use core::str::FromStr;

/// 48-bit MAC (Media Access Control) address
///
/// Represents a unique hardware address for Ethernet network interfaces.
/// Format: 6 bytes, typically displayed as XX:XX:XX:XX:XX:XX in hexadecimal.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    /// Create a new MAC address from 6 bytes
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    /// Broadcast MAC address (FF:FF:FF:FF:FF:FF)
    pub const fn broadcast() -> Self {
        Self([0xFF; 6])
    }

    /// Zero MAC address (00:00:00:00:00:00)
    pub const fn zero() -> Self {
        Self([0x00; 6])
    }

    /// Check if this is a broadcast address
    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xFF; 6]
    }

    /// Check if this is a multicast address (bit 0 of first byte is 1)
    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0x01) != 0
    }

    /// Check if this is a unicast address (not multicast)
    pub fn is_unicast(&self) -> bool {
        !self.is_multicast()
    }

    /// Get the bytes of this MAC address
    pub const fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }
}

/// Parse a MAC address from a colon-separated hex string
///
/// Example: "B8:27:EB:12:34:56"
impl FromStr for MacAddress {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: alloc::vec::Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return Err(());
        }

        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16).map_err(|_| ())?;
        }

        Ok(Self(bytes))
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

/// Ethernet II frame
///
/// Standard Ethernet frame format:
/// - Destination MAC (6 bytes)
/// - Source MAC (6 bytes)
/// - EtherType (2 bytes, big-endian)
/// - Payload (46-1500 bytes)
/// - CRC (4 bytes, typically handled by hardware)
pub struct EthernetFrame<'a> {
    pub dest_mac: MacAddress,
    pub src_mac: MacAddress,
    pub ethertype: u16,
    pub payload: &'a [u8],
}

impl<'a> EthernetFrame<'a> {
    /// Minimum frame size (excluding CRC): 14 byte header + 46 byte min payload
    pub const MIN_FRAME_SIZE: usize = 60;

    /// Maximum frame size (excluding CRC): 14 byte header + 1500 byte max payload
    pub const MAX_FRAME_SIZE: usize = 1514;

    /// Ethernet header size: dest MAC (6) + src MAC (6) + ethertype (2)
    pub const HEADER_SIZE: usize = 14;

    /// Minimum payload size
    pub const MIN_PAYLOAD_SIZE: usize = 46;

    /// Maximum payload size (MTU)
    pub const MAX_PAYLOAD_SIZE: usize = 1500;

    /// Create a new Ethernet frame
    pub fn new(
        dest_mac: MacAddress,
        src_mac: MacAddress,
        ethertype: u16,
        payload: &'a [u8],
    ) -> Self {
        Self {
            dest_mac,
            src_mac,
            ethertype,
            payload,
        }
    }

    /// Parse an Ethernet frame from raw bytes
    ///
    /// Returns None if the buffer is too short or invalid.
    pub fn parse(buffer: &'a [u8]) -> Option<Self> {
        if buffer.len() < Self::HEADER_SIZE {
            return None;
        }

        // Extract destination MAC (bytes 0-5)
        let mut dest_bytes = [0u8; 6];
        dest_bytes.copy_from_slice(&buffer[0..6]);
        let dest_mac = MacAddress(dest_bytes);

        // Extract source MAC (bytes 6-11)
        let mut src_bytes = [0u8; 6];
        src_bytes.copy_from_slice(&buffer[6..12]);
        let src_mac = MacAddress(src_bytes);

        // Extract ethertype (bytes 12-13, big-endian)
        let ethertype = u16::from_be_bytes([buffer[12], buffer[13]]);

        // Payload is everything after the header
        let payload = &buffer[Self::HEADER_SIZE..];

        Some(Self {
            dest_mac,
            src_mac,
            ethertype,
            payload,
        })
    }

    /// Write this frame to a buffer
    ///
    /// Returns the number of bytes written, or None if the buffer is too small.
    pub fn write_to(&self, buffer: &mut [u8]) -> Option<usize> {
        let total_size = Self::HEADER_SIZE + self.payload.len();
        if buffer.len() < total_size {
            return None;
        }

        // Write destination MAC
        buffer[0..6].copy_from_slice(&self.dest_mac.0);

        // Write source MAC
        buffer[6..12].copy_from_slice(&self.src_mac.0);

        // Write ethertype (big-endian)
        let ethertype_bytes = self.ethertype.to_be_bytes();
        buffer[12..14].copy_from_slice(&ethertype_bytes);

        // Write payload
        buffer[Self::HEADER_SIZE..total_size].copy_from_slice(self.payload);

        Some(total_size)
    }

    /// Get the total size of this frame when serialized
    pub fn size(&self) -> usize {
        Self::HEADER_SIZE + self.payload.len()
    }
}

// EtherType constants (big-endian values)
// Source: IEEE 802 Numbers
// <https://www.iana.org/assignments/ieee-802-numbers/ieee-802-numbers.xhtml>

/// IPv4 protocol
pub const ETHERTYPE_IPV4: u16 = 0x0800;

/// ARP (Address Resolution Protocol)
pub const ETHERTYPE_ARP: u16 = 0x0806;

/// IPv6 protocol
pub const ETHERTYPE_IPV6: u16 = 0x86DD;

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    use alloc::string::ToString;

    #[test_case]
    fn test_mac_address_new() {
        let mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        assert_eq!(mac.0, [0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
    }

    #[test_case]
    fn test_mac_address_broadcast() {
        let mac = MacAddress::broadcast();
        assert!(mac.is_broadcast());
        assert_eq!(mac.0, [0xFF; 6]);
    }

    #[test_case]
    fn test_mac_address_zero() {
        let mac = MacAddress::zero();
        assert_eq!(mac.0, [0x00; 6]);
        assert!(!mac.is_broadcast());
    }

    #[test_case]
    fn test_mac_address_multicast() {
        // Multicast addresses have bit 0 of first byte set
        let multicast = MacAddress::new([0x01, 0x00, 0x5E, 0x00, 0x00, 0x01]);
        assert!(multicast.is_multicast());
        assert!(!multicast.is_unicast());

        // Unicast addresses have bit 0 of first byte clear
        let unicast = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        assert!(!unicast.is_multicast());
        assert!(unicast.is_unicast());

        // Broadcast is also multicast
        let broadcast = MacAddress::broadcast();
        assert!(broadcast.is_multicast());
    }

    #[test_case]
    fn test_mac_address_display() {
        let mac = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        assert_eq!(mac.to_string(), "B8:27:EB:12:34:56");

        let mac2 = MacAddress::new([0x00, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E]);
        assert_eq!(mac2.to_string(), "00:0A:0B:0C:0D:0E");
    }

    #[test_case]
    fn test_mac_address_from_str() {
        let mac: MacAddress = "B8:27:EB:12:34:56".parse().unwrap();
        assert_eq!(mac.0, [0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);

        let mac2: MacAddress = "00:0a:0b:0c:0d:0e".parse().unwrap();
        assert_eq!(mac2.0, [0x00, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E]);

        // Invalid formats should return Err
        assert!("invalid".parse::<MacAddress>().is_err());
        assert!("B8:27:EB:12:34".parse::<MacAddress>().is_err()); // Too short
        assert!("B8:27:EB:12:34:56:78".parse::<MacAddress>().is_err()); // Too long
        assert!("ZZ:27:EB:12:34:56".parse::<MacAddress>().is_err()); // Invalid hex
    }

    #[test_case]
    fn test_ethernet_frame_parse() {
        // Construct a minimal frame
        let mut buffer = [0u8; 64];

        // Destination MAC: FF:FF:FF:FF:FF:FF (broadcast)
        buffer[0..6].copy_from_slice(&[0xFF; 6]);

        // Source MAC: B8:27:EB:12:34:56
        buffer[6..12].copy_from_slice(&[0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);

        // EtherType: 0x0806 (ARP)
        buffer[12..14].copy_from_slice(&[0x08, 0x06]);

        // Payload: some test data
        buffer[14..20].copy_from_slice(b"Hello!");

        let frame = EthernetFrame::parse(&buffer[..20]).unwrap();

        assert_eq!(frame.dest_mac, MacAddress::broadcast());
        assert_eq!(
            frame.src_mac,
            MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56])
        );
        assert_eq!(frame.ethertype, ETHERTYPE_ARP);
        assert_eq!(frame.payload, b"Hello!");
        assert_eq!(frame.size(), 20);
    }

    #[test_case]
    fn test_ethernet_frame_parse_too_short() {
        let buffer = [0u8; 10]; // Less than header size
        assert!(EthernetFrame::parse(&buffer).is_none());
    }

    #[test_case]
    fn test_ethernet_frame_write() {
        let dest = MacAddress::broadcast();
        let src = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        let payload = b"Test payload";

        let frame = EthernetFrame::new(dest, src, ETHERTYPE_IPV4, payload);

        let mut buffer = [0u8; 128];
        let size = frame.write_to(&mut buffer).unwrap();

        assert_eq!(size, 14 + 12); // Header + payload

        // Verify destination MAC
        assert_eq!(&buffer[0..6], &[0xFF; 6]);

        // Verify source MAC
        assert_eq!(&buffer[6..12], &[0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);

        // Verify ethertype (big-endian 0x0800)
        assert_eq!(&buffer[12..14], &[0x08, 0x00]);

        // Verify payload
        assert_eq!(&buffer[14..26], b"Test payload");
    }

    #[test_case]
    fn test_ethernet_frame_write_buffer_too_small() {
        let dest = MacAddress::broadcast();
        let src = MacAddress::new([0xB8, 0x27, 0xEB, 0x12, 0x34, 0x56]);
        let payload = b"Test payload";

        let frame = EthernetFrame::new(dest, src, ETHERTYPE_IPV4, payload);

        let mut buffer = [0u8; 10]; // Too small
        assert!(frame.write_to(&mut buffer).is_none());
    }

    #[test_case]
    fn test_ethernet_frame_roundtrip() {
        let dest = MacAddress::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        let src = MacAddress::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let payload = b"Roundtrip test data";

        let frame = EthernetFrame::new(dest, src, ETHERTYPE_ARP, payload);

        let mut buffer = [0u8; 128];
        let size = frame.write_to(&mut buffer).unwrap();

        // Parse it back
        let parsed = EthernetFrame::parse(&buffer[..size]).unwrap();

        assert_eq!(parsed.dest_mac, dest);
        assert_eq!(parsed.src_mac, src);
        assert_eq!(parsed.ethertype, ETHERTYPE_ARP);
        assert_eq!(parsed.payload, payload);
    }

    #[test_case]
    fn test_ethertype_constants() {
        // Verify the constants are in big-endian format
        assert_eq!(ETHERTYPE_IPV4, 0x0800);
        assert_eq!(ETHERTYPE_ARP, 0x0806);
        assert_eq!(ETHERTYPE_IPV6, 0x86DD);
    }
}

//! Property Mailbox Channel
//!
//! High-level interface for the VideoCore property mailbox (channel 8).
//! This channel uses structured messages with tags to query system properties
//! like MAC address, board serial, firmware version, etc.
//!
//! # Protocol
//!
//! Messages consist of:
//! 1. Header (buffer size + request/response code)
//! 2. One or more tags (each with tag ID + size + data)
//! 3. End tag (0x00000000)
//!
//! The buffer must be 16-byte aligned and passed as a physical address.
//!
//! # References
//!
//! - U-Boot: arch/arm/mach-bcm283x/include/mach/mbox.h
//! - Firmware wiki: <https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface>

use super::videocore::{Mailbox, MailboxError};

/// Property channel ID
const PROPERTY_CHANNEL: u32 = 8;

/// Request code (sent by ARM)
const REQUEST_CODE: u32 = 0x0000_0000;

/// Success response code (set by VideoCore)
const RESPONSE_CODE_SUCCESS: u32 = 0x8000_0000;

/// Response bit in tag val_len field
const TAG_RESPONSE_BIT: u32 = 0x8000_0000;

/// Property mailbox tag IDs
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum PropertyTag {
    /// Get board MAC address (0x00010003)
    GetMacAddress = 0x0001_0003,
    /// Get board serial number (0x00010004)
    GetBoardSerial = 0x0001_0004,
    /// Get ARM memory region (0x00010005)
    GetArmMemory = 0x0001_0005,
    /// Get firmware revision (0x00000001)
    GetFirmwareRevision = 0x0000_0001,
}

/// Property mailbox errors
#[derive(Debug, Clone, Copy)]
pub enum PropertyError {
    /// Underlying mailbox error
    Mailbox(MailboxError),
    /// Response code indicates failure
    ResponseFailed,
    /// Tag response bit not set
    TagNotResponded,
    /// Buffer not 16-byte aligned
    NotAligned,
}

/// Message buffer header
#[repr(C)]
struct MessageHeader {
    buf_size: u32, // Total buffer size in bytes
    code: u32,     // Request (0) or response (0x80000000)
}

/// Tag header (precedes tag-specific data)
#[repr(C)]
struct TagHeader {
    tag: u32,      // Tag ID
    buf_size: u32, // Size of value buffer in bytes
    val_len: u32,  // Request: size of request data; Response: MSB set + response data size
}

/// MAC address request/response
/// Aligned to cache line size (64 bytes) as required by VideoCore firmware
#[repr(C, align(64))]
struct MacAddressMessage {
    header: MessageHeader,
    tag_hdr: TagHeader,
    mac: [u8; 6],
    _pad: [u8; 2], // Padding to 8 bytes
    end_tag: u32,  // 0x00000000 terminator
}

/// Board serial request/response
/// Aligned to cache line size (64 bytes) as required by VideoCore firmware
#[repr(C, align(64))]
struct BoardSerialMessage {
    header: MessageHeader,
    tag_hdr: TagHeader,
    serial: u64,
    end_tag: u32,
}

/// Firmware revision request/response
/// Aligned to cache line size (64 bytes) as required by VideoCore firmware
#[repr(C, align(64))]
struct FirmwareRevisionMessage {
    header: MessageHeader,
    tag_hdr: TagHeader,
    revision: u32,
    end_tag: u32,
}

/// Property mailbox interface
pub struct PropertyMailbox {
    mbox: Mailbox,
}

#[allow(clippy::new_without_default)] // Hardware interface shouldn't have Default - explicit new() is clearer
impl PropertyMailbox {
    /// Create new property mailbox interface
    pub const fn new() -> Self {
        Self {
            mbox: Mailbox::new(),
        }
    }

    /// Flush cache for message buffer before sending to VideoCore
    #[inline]
    fn flush_cache<T>(buffer: &T) {
        let addr = buffer as *const T as usize;
        let size = core::mem::size_of::<T>();

        // Flush full cache lines
        let cache_line_size = 64; // ARM Cortex-A72
        let start = addr & !(cache_line_size - 1);
        let end = (addr + size + cache_line_size - 1) & !(cache_line_size - 1);

        // SAFETY: Flushing cache for valid buffer
        unsafe {
            let mut line = start;
            while line < end {
                core::arch::asm!(
                    "dc cvac, {addr}",
                    addr = in(reg) line,
                    options(nostack)
                );
                line += cache_line_size;
            }
            core::arch::asm!("dsb sy", options(nostack));
        }
    }

    /// Invalidate cache for message buffer after VideoCore modifies it
    #[inline]
    fn invalidate_cache<T>(buffer: &T) {
        let addr = buffer as *const T as usize;
        let size = core::mem::size_of::<T>();

        let cache_line_size = 64; // ARM Cortex-A72
        let start = addr & !(cache_line_size - 1);
        let end = (addr + size + cache_line_size - 1) & !(cache_line_size - 1);

        // SAFETY: Invalidating cache for valid buffer
        unsafe {
            core::arch::asm!("dsb sy", options(nostack));
            let mut line = start;
            while line < end {
                core::arch::asm!(
                    "dc ivac, {addr}",
                    addr = in(reg) line,
                    options(nostack)
                );
                line += cache_line_size;
            }
            core::arch::asm!("dsb sy", options(nostack));
        }
    }

    /// Get MAC address from VideoCore firmware
    ///
    /// The firmware reads the MAC address from OTP memory and returns it.
    /// On Raspberry Pi 4, this is the MAC address programmed during manufacturing.
    ///
    /// # Returns
    ///
    /// 6-byte MAC address array
    ///
    /// # Errors
    ///
    /// Returns error if mailbox communication fails or firmware doesn't respond
    pub fn get_mac_address(&self) -> Result<[u8; 6], PropertyError> {
        // Note: Message is modified in-place by VideoCore firmware via DMA
        // Rust doesn't see the modification (happens through cache invalidation)
        #[allow(unused_mut)]
        let mut msg = MacAddressMessage {
            header: MessageHeader {
                buf_size: core::mem::size_of::<MacAddressMessage>() as u32,
                code: REQUEST_CODE,
            },
            tag_hdr: TagHeader {
                tag: PropertyTag::GetMacAddress as u32,
                buf_size: 8, // 6 bytes MAC + 2 bytes padding
                val_len: 0,  // Request has no input data
            },
            mac: [0u8; 6],
            _pad: [0u8; 2],
            end_tag: 0,
        };

        // Flush cache so VideoCore sees our request
        Self::flush_cache(&msg);

        // Send buffer address to VideoCore
        let buffer_phys = &msg as *const _ as u32;

        // Verify alignment (must be 16-byte aligned at minimum)
        if (buffer_phys & 0xF) != 0 {
            return Err(PropertyError::NotAligned);
        }

        // Convert ARM physical address to VideoCore bus address
        // Source: U-Boot arch/arm/mach-bcm283x/phys2bus.c
        // On BCM2711, VideoCore accesses DRAM at 0xC0000000 + phys_addr
        let buffer_addr = 0xC000_0000 | buffer_phys;

        let response_addr = self
            .mbox
            .call(PROPERTY_CHANNEL, buffer_addr)
            .map_err(PropertyError::Mailbox)?;

        // Validate response address matches what we sent
        if response_addr != buffer_addr {
            return Err(PropertyError::ResponseFailed);
        }

        // Invalidate cache so we see VideoCore's response
        Self::invalidate_cache(&msg);

        // Validate response header
        if msg.header.code != RESPONSE_CODE_SUCCESS {
            return Err(PropertyError::ResponseFailed);
        }

        // Validate tag response bit
        if (msg.tag_hdr.val_len & TAG_RESPONSE_BIT) == 0 {
            return Err(PropertyError::TagNotResponded);
        }

        Ok(msg.mac)
    }

    /// Get board serial number from VideoCore firmware
    ///
    /// Returns the 64-bit board serial number programmed during manufacturing.
    pub fn get_board_serial(&self) -> Result<u64, PropertyError> {
        // Note: Message is modified in-place by VideoCore firmware via DMA
        #[allow(unused_mut)]
        let mut msg = BoardSerialMessage {
            header: MessageHeader {
                buf_size: core::mem::size_of::<BoardSerialMessage>() as u32,
                code: REQUEST_CODE,
            },
            tag_hdr: TagHeader {
                tag: PropertyTag::GetBoardSerial as u32,
                buf_size: 8,
                val_len: 0,
            },
            serial: 0,
            end_tag: 0,
        };

        Self::flush_cache(&msg);

        let buffer_phys = &msg as *const _ as u32;
        if (buffer_phys & 0xF) != 0 {
            return Err(PropertyError::NotAligned);
        }

        // Convert to VideoCore bus address
        let buffer_addr = 0xC000_0000 | buffer_phys;

        let response_addr = self
            .mbox
            .call(PROPERTY_CHANNEL, buffer_addr)
            .map_err(PropertyError::Mailbox)?;

        if response_addr != buffer_addr {
            return Err(PropertyError::ResponseFailed);
        }

        Self::invalidate_cache(&msg);

        if msg.header.code != RESPONSE_CODE_SUCCESS {
            return Err(PropertyError::ResponseFailed);
        }

        if (msg.tag_hdr.val_len & TAG_RESPONSE_BIT) == 0 {
            return Err(PropertyError::TagNotResponded);
        }

        Ok(msg.serial)
    }

    /// Get firmware revision from VideoCore
    ///
    /// Returns the VideoCore firmware revision number.
    pub fn get_firmware_revision(&self) -> Result<u32, PropertyError> {
        // Note: Message is modified in-place by VideoCore firmware via DMA
        #[allow(unused_mut)]
        let mut msg = FirmwareRevisionMessage {
            header: MessageHeader {
                buf_size: core::mem::size_of::<FirmwareRevisionMessage>() as u32,
                code: REQUEST_CODE,
            },
            tag_hdr: TagHeader {
                tag: PropertyTag::GetFirmwareRevision as u32,
                buf_size: 4,
                val_len: 0,
            },
            revision: 0,
            end_tag: 0,
        };

        Self::flush_cache(&msg);

        let buffer_phys = &msg as *const _ as u32;
        if (buffer_phys & 0xF) != 0 {
            return Err(PropertyError::NotAligned);
        }

        // Convert to VideoCore bus address
        let buffer_addr = 0xC000_0000 | buffer_phys;

        let response_addr = self
            .mbox
            .call(PROPERTY_CHANNEL, buffer_addr)
            .map_err(PropertyError::Mailbox)?;

        if response_addr != buffer_addr {
            return Err(PropertyError::ResponseFailed);
        }

        Self::invalidate_cache(&msg);

        if msg.header.code != RESPONSE_CODE_SUCCESS {
            return Err(PropertyError::ResponseFailed);
        }

        if (msg.tag_hdr.val_len & TAG_RESPONSE_BIT) == 0 {
            return Err(PropertyError::TagNotResponded);
        }

        Ok(msg.revision)
    }
}

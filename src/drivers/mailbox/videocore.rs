//! VideoCore Mailbox Hardware Interface
//!
//! Low-level driver for the BCM2835/BCM2711 mailbox hardware used to
//! communicate with the VideoCore GPU firmware.
//!
//! # Protocol
//!
//! The mailbox passes 32-bit values where:
//! - Lower 4 bits: channel ID (0-15)
//! - Upper 28 bits: data payload (must be 16-byte aligned address for property channel)
//!
//! # Hardware Operation
//!
//! 1. Wait for mailbox to not be full (check STATUS register)
//! 2. Write (channel | data) to WRITE register
//! 3. Wait for response available (check STATUS register)
//! 4. Read response from READ register
//! 5. Validate channel matches
//!
//! # References
//!
//! - U-Boot: arch/arm/mach-bcm283x/mbox.c
//! - BCM2711 ARM Peripherals §1.3

use crate::drivers::clocksource::SystemTimer;

/// Mailbox base address (BCM2711)
/// Source: BCM2711 ARM Peripherals §1.3
const MAILBOX_BASE: usize = 0xFE00_B880;

/// Mailbox hardware registers
#[repr(C)]
struct MailboxRegisters {
    read: u32, // 0x00: Mailbox 0 read (ARM receives)
    _reserved0: [u32; 5],
    status0: u32,  // 0x18: Mailbox 0 status
    _config0: u32, // 0x1C: Mailbox 0 config
    write: u32,    // 0x20: Mailbox 1 write (ARM sends)
    _reserved1: [u32; 5],
    status1: u32,  // 0x38: Mailbox 1 status
    _config1: u32, // 0x3C: Mailbox 1 config
}

/// Mailbox status flags
const STATUS_READ_EMPTY: u32 = 0x4000_0000;
const STATUS_WRITE_FULL: u32 = 0x8000_0000;

/// Channel mask (lower 4 bits)
const CHANNEL_MASK: u32 = 0xF;

/// Timeout for mailbox operations (milliseconds)
const TIMEOUT_MS: u32 = 1000;

/// Mailbox error types
#[derive(Debug, Clone, Copy)]
pub enum MailboxError {
    /// Timeout while draining stale responses
    DrainTimeout,
    /// Timeout waiting for space to send
    SendTimeout,
    /// Timeout waiting for response
    ReceiveTimeout,
    /// Response channel mismatch
    ChannelMismatch,
    /// Invalid data (lower 4 bits not zero)
    InvalidData,
}

/// Low-level mailbox hardware driver
pub struct Mailbox {
    base_addr: usize,
}

#[allow(clippy::new_without_default)] // Hardware drivers shouldn't have Default - explicit new() is clearer
impl Mailbox {
    /// Create new mailbox instance
    pub const fn new() -> Self {
        Self {
            base_addr: MAILBOX_BASE,
        }
    }

    /// Get pointer to mailbox registers
    fn regs(&self) -> &'static mut MailboxRegisters {
        // SAFETY: Mailbox registers are memory-mapped at valid MMIO address
        unsafe { &mut *(self.base_addr as *mut MailboxRegisters) }
    }

    /// Read a register with volatile semantics
    #[inline]
    fn read_reg(&self, reg: &u32) -> u32 {
        let addr = reg as *const u32;
        // SAFETY: Reading from valid MMIO register
        unsafe { core::ptr::read_volatile(addr) }
    }

    /// Write a register with volatile semantics and memory barrier
    #[inline]
    fn write_reg(&self, reg: &mut u32, value: u32) {
        let addr = reg as *mut u32;
        // SAFETY: Data Memory Barrier ensures ordering
        unsafe {
            core::arch::asm!("dmb sy", options(nostack));
            core::ptr::write_volatile(addr, value);
        }
    }

    /// Pack channel and data into mailbox value
    /// Data must have lower 4 bits clear (16-byte aligned for property channel)
    #[inline]
    fn pack(channel: u32, data: u32) -> u32 {
        (data & !CHANNEL_MASK) | (channel & CHANNEL_MASK)
    }

    /// Extract channel from mailbox value
    #[inline]
    fn unpack_channel(value: u32) -> u32 {
        value & CHANNEL_MASK
    }

    /// Extract data from mailbox value
    #[inline]
    fn unpack_data(value: u32) -> u32 {
        value & !CHANNEL_MASK
    }

    /// Send raw mailbox message and receive response
    ///
    /// # Arguments
    ///
    /// * `channel` - Mailbox channel (0-15)
    /// * `data` - Data to send (must be 16-byte aligned, lower 4 bits zero)
    ///
    /// # Returns
    ///
    /// The response data (upper 28 bits of received value)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Data has lower 4 bits set (not aligned)
    /// - Timeout occurs during any operation
    /// - Response channel doesn't match request channel
    pub fn call(&self, channel: u32, data: u32) -> Result<u32, MailboxError> {
        let regs = self.regs();

        // Validate data alignment
        if (data & CHANNEL_MASK) != 0 {
            return Err(MailboxError::InvalidData);
        }

        // Drain any stale responses
        let start = SystemTimer::timestamp_us();
        loop {
            let status = self.read_reg(&regs.status0);
            if (status & STATUS_READ_EMPTY) != 0 {
                break;
            }
            if SystemTimer::timestamp_us() - start > (TIMEOUT_MS as u64 * 1000) {
                return Err(MailboxError::DrainTimeout);
            }
            let _ = self.read_reg(&regs.read); // Drain one message
        }

        // Wait for space to send
        let start = SystemTimer::timestamp_us();
        loop {
            let status = self.read_reg(&regs.status1);
            if (status & STATUS_WRITE_FULL) == 0 {
                break;
            }
            if SystemTimer::timestamp_us() - start > (TIMEOUT_MS as u64 * 1000) {
                return Err(MailboxError::SendTimeout);
            }
        }

        // Send the request
        let value = Self::pack(channel, data);
        self.write_reg(&mut regs.write, value);

        // Wait for response
        let start = SystemTimer::timestamp_us();
        loop {
            let status = self.read_reg(&regs.status0);
            if (status & STATUS_READ_EMPTY) == 0 {
                break;
            }
            if SystemTimer::timestamp_us() - start > (TIMEOUT_MS as u64 * 1000) {
                return Err(MailboxError::ReceiveTimeout);
            }
        }

        // Read the response
        let response = self.read_reg(&regs.read);

        // Validate channel
        if Self::unpack_channel(response) != channel {
            return Err(MailboxError::ChannelMismatch);
        }

        Ok(Self::unpack_data(response))
    }
}

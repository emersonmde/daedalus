//! VideoCore Mailbox Interface
//!
//! The BCM2711 SoC contains the VideoCore GPU which controls initial boot
//! and provides various system services via a mailbox protocol. The mailbox
//! hardware supports passing 32-bit messages between the ARM CPU and VideoCore.
//!
//! The property channel (channel 8) is used for structured requests like
//! querying the MAC address, serial number, firmware version, etc.
//!
//! # References
//!
//! - U-Boot: arch/arm/mach-bcm283x/mbox.c
//! - Firmware docs: <https://github.com/raspberrypi/firmware/wiki/Mailboxes>

pub mod property;
pub mod videocore;

pub use property::PropertyMailbox;
pub use videocore::Mailbox;

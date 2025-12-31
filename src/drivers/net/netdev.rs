//! Network Device Abstraction
//!
//! This module provides a trait for network device drivers, enabling support
//! for multiple hardware implementations (GENET on Pi 4, future Pi 5, QEMU mock).
//!
//! ## Design Philosophy
//!
//! The `NetworkDevice` trait abstracts the minimal interface needed for Ethernet
//! frame transmission and reception. This allows:
//!
//! - **Hardware portability**: Support Raspberry Pi 4 (GENET), Pi 5 (future), and mock devices
//! - **Testing**: QEMU mock driver enables protocol testing without hardware
//! - **Integration**: Clean interface for smoltcp TCP/IP stack integration
//!
//! ## Current Implementations
//!
//! - `GenetController` - BCM2711 GENET v5 Ethernet controller (Pi 4)
//!
//! ## Future Implementations
//!
//! - Mock device for QEMU testing (Milestone #14)
//! - Pi 5 Ethernet controller (when hardware available)
//!
//! ## Example Usage
//!
//! ```ignore
//! use daedalus::drivers::netdev::NetworkDevice;
//! use daedalus::drivers::genet::GenetController;
//!
//! // Create device
//! let mut netdev = GenetController::new();
//!
//! // Check if hardware present (returns false in QEMU)
//! if netdev.is_present() {
//!     // Initialize device
//!     netdev.init().expect("Failed to initialize network device");
//!
//!     // Get MAC address
//!     let mac = netdev.mac_address();
//!     // MAC address is now available for use
//!
//!     // Transmit frame
//!     let frame = [0xFF; 64]; // Broadcast frame
//!     netdev.transmit(&frame).expect("Failed to send frame");
//!
//!     // Receive frame (non-blocking)
//!     if let Some(frame) = netdev.receive() {
//!         // Process received frame (frame.len() bytes)
//!     }
//! }
//! ```

use crate::net::ethernet::MacAddress;
use core::fmt;

/// Errors that can occur during network device operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkError {
    /// Hardware is not present or not responding
    HardwareNotPresent,

    /// Device is not initialized
    NotInitialized,

    /// Transmit buffer is full (try again later)
    TxBufferFull,

    /// Frame is too large for device
    FrameTooLarge,

    /// Frame is too small (below minimum Ethernet frame size)
    FrameTooSmall,

    /// Hardware error during operation
    HardwareError,

    /// Timeout waiting for operation to complete
    Timeout,

    /// Timeout waiting for transmission to complete
    TransmitTimeout,

    /// Invalid configuration or parameter
    InvalidConfiguration,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::HardwareNotPresent => write!(f, "Hardware not present"),
            NetworkError::NotInitialized => write!(f, "Device not initialized"),
            NetworkError::TxBufferFull => write!(f, "Transmit buffer full"),
            NetworkError::FrameTooLarge => write!(f, "Frame too large"),
            NetworkError::FrameTooSmall => write!(f, "Frame too small"),
            NetworkError::HardwareError => write!(f, "Hardware error"),
            NetworkError::Timeout => write!(f, "Operation timeout"),
            NetworkError::TransmitTimeout => write!(f, "Transmit timeout"),
            NetworkError::InvalidConfiguration => write!(f, "Invalid configuration"),
        }
    }
}

/// Network device abstraction trait
///
/// This trait defines the minimal interface for Ethernet network devices.
/// Implementations provide hardware-specific details for frame TX/RX.
///
/// # Design Decisions
///
/// - **Blocking transmit**: Simplifies initial implementation (interrupts come later)
/// - **Non-blocking receive**: Check for frames without waiting
/// - **Single-frame API**: No complex queue management in trait
/// - **Result types**: Clear error handling for hardware issues
///
/// # Thread Safety
///
/// Implementations are not required to be thread-safe at the trait level.
/// Synchronization must be handled by the caller (e.g., wrapping in `Mutex`).
/// Individual implementations may use lock-free techniques internally (e.g., DMA rings).
pub trait NetworkDevice {
    /// Check if the hardware is present and accessible
    ///
    /// This is used to detect when running in QEMU (no hardware) vs real Pi 4.
    /// Implementations should safely probe for hardware without causing exceptions.
    ///
    /// # Returns
    ///
    /// - `true` if hardware is detected and accessible
    /// - `false` if hardware is not present (e.g., running in QEMU)
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use daedalus::drivers::genet::GenetController;
    /// # use daedalus::drivers::netdev::NetworkDevice;
    /// let netdev = GenetController::new();
    /// if netdev.is_present() {
    ///     // Network hardware detected - can initialize
    /// } else {
    ///     // No network hardware (running in QEMU?)
    /// }
    /// ```
    fn is_present(&self) -> bool;

    /// Initialize the network device
    ///
    /// Performs all necessary hardware initialization:
    /// - Reset and configure MAC controller
    /// - Configure PHY (if present)
    /// - Set up TX/RX buffers
    /// - Configure interrupts (if using interrupt-driven mode)
    ///
    /// This must be called before `transmit()` or `receive()`.
    ///
    /// # Errors
    ///
    /// - `HardwareNotPresent` - Device not detected
    /// - `HardwareError` - Initialization sequence failed
    /// - `Timeout` - PHY or MAC configuration timed out
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use daedalus::drivers::genet::GenetController;
    /// # use daedalus::drivers::netdev::NetworkDevice;
    /// let mut netdev = GenetController::new();
    /// netdev.init().expect("Failed to initialize");
    /// ```
    #[must_use = "init() failure must be handled - device may not be operational"]
    fn init(&mut self) -> Result<(), NetworkError>;

    /// Transmit an Ethernet frame
    ///
    /// Sends a complete Ethernet frame (including header). The frame should be
    /// a valid Ethernet II frame with destination MAC, source MAC, EtherType,
    /// and payload. The CRC is typically calculated by hardware.
    ///
    /// This is a **blocking** operation - it waits until the frame is queued
    /// for transmission. It does NOT wait for transmission to complete.
    ///
    /// # Arguments
    ///
    /// - `frame` - Complete Ethernet frame (14-byte header + payload)
    ///
    /// # Frame Size Constraints
    ///
    /// - Minimum: 64 bytes (includes header + payload, excludes 4-byte CRC)
    /// - Maximum: 1514 bytes (includes header + payload, excludes 4-byte CRC)
    ///
    /// Note: IEEE 802.3 minimum is 64 bytes including 4-byte CRC (60 bytes payload).
    /// Hardware typically handles CRC, so software passes 64-byte frames minimum.
    ///
    /// # Errors
    ///
    /// - `NotInitialized` - Must call `init()` first
    /// - `FrameTooSmall` - Frame < 64 bytes
    /// - `FrameTooLarge` - Frame > 1514 bytes
    /// - `TxBufferFull` - Hardware buffer full, try again
    /// - `HardwareError` - Transmission failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use daedalus::drivers::genet::GenetController;
    /// # use daedalus::drivers::netdev::NetworkDevice;
    /// # use daedalus::net::ethernet::*;
    /// # let mut netdev = GenetController::new();
    /// # netdev.init().unwrap();
    /// // Build frame
    /// let frame = EthernetFrame::new(
    ///     MacAddress::broadcast(),
    ///     MacAddress::new([0xB8, 0x27, 0xEB, 1, 2, 3]),
    ///     ETHERTYPE_ARP,
    ///     &[0u8; 46],
    /// );
    ///
    /// // Serialize and send
    /// let mut buffer = [0u8; 1518];
    /// let size = frame.write_to(&mut buffer).unwrap();
    /// netdev.transmit(&buffer[..size]).expect("Send failed");
    /// ```
    #[must_use = "transmit() failure must be handled - frame may not have been sent"]
    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError>;

    /// Receive an Ethernet frame (non-blocking)
    ///
    /// Checks if a frame has been received and returns it. This is non-blocking:
    /// if no frame is available, returns `None` immediately.
    ///
    /// The returned slice is valid until the next call to `receive()` (the
    /// implementation may reuse an internal buffer).
    ///
    /// # Returns
    ///
    /// - `Some(&[u8])` - Received frame data
    /// - `None` - No frame available
    ///
    /// # Frame Validation
    ///
    /// Implementations should filter out invalid frames (bad CRC, runt frames)
    /// before returning them to the caller.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use daedalus::drivers::genet::NetworkDevice;
    /// # use daedalus::drivers::genet::GenetController;
    /// # use daedalus::net::ethernet::*;
    /// # let mut netdev = GenetController::new();
    /// # netdev.init().unwrap();
    /// // Poll for frames
    /// if let Some(frame_data) = netdev.receive() {
    ///     if let Some(frame) = EthernetFrame::parse(frame_data) {
    ///         // Process frame from frame.src_mac
    ///     }
    /// }
    /// ```
    fn receive(&mut self) -> Option<&[u8]>;

    /// Get the device's MAC address
    ///
    /// Returns the hardware MAC address for this device. On Raspberry Pi,
    /// this is typically read from OTP (One-Time Programmable) memory.
    ///
    /// # Returns
    ///
    /// The 48-bit MAC address for this device.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use daedalus::drivers::genet::GenetController;
    /// # use daedalus::drivers::netdev::NetworkDevice;
    /// # let mut netdev = GenetController::new();
    /// # netdev.init().unwrap();
    /// let mac = netdev.mac_address();
    /// // MAC address retrieved successfully
    /// ```
    fn mac_address(&self) -> MacAddress;

    /// Get link status (optional, returns false by default)
    ///
    /// Checks if the network link is up (cable connected, PHY negotiated).
    /// Not all devices support link detection.
    ///
    /// # Returns
    ///
    /// - `true` if link is up and operational
    /// - `false` if link is down or unsupported
    ///
    /// # Default Implementation
    ///
    /// Returns `false` (link down). Devices with PHYs should override this.
    fn link_up(&self) -> bool {
        false
    }

    /// Free the RX buffer after processing a received frame
    ///
    /// This must be called after processing a frame returned by `receive()`.
    /// It tells the driver that the buffer can be reused for receiving new frames.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use daedalus::drivers::genet::GenetController;
    /// # use daedalus::drivers::netdev::NetworkDevice;
    /// # let mut netdev = GenetController::new();
    /// # netdev.init().unwrap();
    /// if let Some(frame) = netdev.receive() {
    ///     // Process frame...
    ///     netdev.free_rx_buffer(); // Mark buffer as free
    /// }
    /// ```
    fn free_rx_buffer(&mut self);
}

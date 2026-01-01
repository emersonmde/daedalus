//! Device drivers subsystem
//!
//! Organized by device class:
//! - `tty`: Terminal and serial drivers
//! - `gpio`: General Purpose I/O controllers
//! - `net`: Network device drivers
//! - `irqchip`: Interrupt controllers
//! - `clocksource`: Timers and clock sources
//! - `mailbox`: VideoCore mailbox interface
//!
//! See ADR-004 for filesystem structure rationale.

pub mod clocksource;
pub mod gpio;
pub mod irqchip;
pub mod mailbox;
pub mod net;
pub mod tty;

// Backward compatibility aliases (deprecated - use specific paths instead)
// These will be removed in v0.2.0

/// Deprecated: Use `tty::serial` instead
pub mod uart {
    pub use crate::drivers::tty::serial::*;
}

/// Deprecated: Use `irqchip::gic_v2` instead
pub mod gic {
    pub use crate::drivers::irqchip::gic_v2::*;
}

/// Deprecated: Use `clocksource::bcm2711` instead
pub mod timer {
    pub use crate::drivers::clocksource::bcm2711::*;
}

/// Deprecated: Use `net::ethernet::broadcom::genet` instead
pub mod genet {
    pub use crate::drivers::net::ethernet::broadcom::genet::*;
}

/// Deprecated: Use `net::netdev` instead
pub mod netdev {
    pub use crate::drivers::net::netdev::*;
}

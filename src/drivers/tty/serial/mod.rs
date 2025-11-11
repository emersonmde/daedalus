//! Serial (UART) device drivers

pub mod amba_pl011;

// Re-export for convenience
pub use amba_pl011::*;

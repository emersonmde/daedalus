//! Interrupt controller (irqchip) drivers

pub mod gic_v2;

// Re-export for convenience
pub use gic_v2::*;

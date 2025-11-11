//! Network device drivers and abstractions

pub mod ethernet;
pub mod netdev;

// Re-export NetworkDevice trait for convenience
pub use netdev::*;

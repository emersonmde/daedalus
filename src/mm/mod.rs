//! Memory Management subsystem
//!
//! Provides kernel memory allocation and management facilities.

pub mod allocator;

// Re-export for convenience
pub use allocator::*;

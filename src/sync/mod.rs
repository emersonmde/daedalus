//! Synchronization primitives for DaedalusOS
//!
//! This module provides interrupt-safe and multi-core-safe synchronization.

pub mod mutex;

pub use mutex::Mutex;

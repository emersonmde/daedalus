//! HTTP/0.9 client for network kernel loading
//!
//! Protocol: Send "GET /path\r\n", receive raw binary until connection close.

extern crate alloc;
use crate::println;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy)]
pub enum HttpError {
    ConnectionFailed,
    SendFailed,
    ReceiveFailed,
    ResponseTooLarge,
    NetworkNotReady,
}

/// Fetch kernel binary via HTTP/0.9
pub fn get_binary(server_ip: [u8; 4], port: u16, path: &str) -> Result<Vec<u8>, HttpError> {
    // TODO: Implement with smoltcp TCP stack (requires Device wrapper for GENET)
    println!(
        "HTTP: Would fetch {}.{}.{}.{}:{}{}",
        server_ip[0], server_ip[1], server_ip[2], server_ip[3], port, path
    );
    Err(HttpError::NetworkNotReady)
}

//! HTTP/0.9 client for fetching kernel binaries
//!
//! This module provides a minimal HTTP client implementation using smoltcp's TCP stack.
//! It implements HTTP/0.9, the simplest HTTP protocol:
//! - Send: "GET /path\r\n"
//! - Receive: raw binary response until connection close
//! - No headers, no status codes, just raw data
//!
//! This is sufficient for fetching kernel binaries from the development server.

extern crate alloc;
use alloc::vec::Vec;
use crate::println;

/// HTTP client errors
#[derive(Debug, Clone, Copy)]
pub enum HttpError {
    /// TCP connection failed
    ConnectionFailed,
    /// Failed to send HTTP request
    SendFailed,
    /// Failed to receive response
    ReceiveFailed,
    /// Response too large (> 32 MB)
    ResponseTooLarge,
    /// Network not initialized
    NetworkNotReady,
}

/// Fetch a binary file via HTTP/0.9
///
/// This performs a simple HTTP GET request and returns the raw binary response.
/// The connection is closed by the server after sending the complete response.
///
/// # Arguments
/// - `server_ip`: IPv4 address as [u8; 4], e.g., [10, 42, 10, 100]
/// - `port`: Server port (typically 8000 for dev server)
/// - `path`: Request path, e.g., "/kernel"
///
/// # Returns
/// - `Ok(Vec<u8>)`: Binary response data
/// - `Err(HttpError)`: Connection or protocol error
///
/// # Example
/// ```ignore
/// let kernel = http::get_binary([10, 42, 10, 100], 8000, "/kernel")?;
/// println!("Downloaded {} bytes", kernel.len());
/// ```
///
/// # Protocol
/// HTTP/0.9 is extremely simple:
/// ```text
/// Client: GET /kernel\r\n
/// Server: <binary data><close connection>
/// ```
pub fn get_binary(
    server_ip: [u8; 4],
    port: u16,
    path: &str,
) -> Result<Vec<u8>, HttpError> {
    // TODO: Implement HTTP client using smoltcp
    //
    // Implementation plan:
    // 1. Create smoltcp Interface with our GENET device
    // 2. Create TCP socket
    // 3. Connect to server_ip:port
    // 4. Send "GET {path}\r\n"
    // 5. Receive data until connection close
    // 6. Return buffer
    //
    // Challenges:
    // - Smoltcp needs a Device implementation wrapping GENET
    // - Need to integrate smoltcp's poll loop with our event system
    // - Need to manage socket/interface lifetime
    //
    // For now, return error - will implement in hardware testing phase

    println!("HTTP: Would fetch {}.{}.{}.{}:{}{}",
        server_ip[0], server_ip[1], server_ip[2], server_ip[3],
        port, path);

    Err(HttpError::NetworkNotReady)
}

/// Format HTTP/0.9 GET request
///
/// HTTP/0.9 format: "GET /path\r\n"
fn format_http_request(path: &str) -> alloc::string::String {
    alloc::format!("GET {}\r\n", path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_http_request_format() {
        let req = format_http_request("/kernel");
        assert_eq!(req, "GET /kernel\r\n");
    }

    #[test_case]
    fn test_http_request_format_root() {
        let req = format_http_request("/");
        assert_eq!(req, "GET /\r\n");
    }

    #[test_case]
    fn test_http_request_format_nested_path() {
        let req = format_http_request("/api/v1/kernel");
        assert_eq!(req, "GET /api/v1/kernel\r\n");
    }
}

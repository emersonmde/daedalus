//! DaedalusOS Development Server
//!
//! Serves kernel binaries over HTTP for network boot development.
//!
//! # Usage
//!
//! ```bash
//! # Serve kernel on port 8000
//! daedalus-dev-server serve path/to/kernel8.img
//!
//! # Run command on Pi via network shell (future)
//! daedalus-dev-server cmd 10.42.10.42:8080 meminfo
//! ```
//!
//! # Protocol
//!
//! HTTP/0.9 - Simplest possible HTTP for embedded clients:
//! ```text
//! Client sends: GET /kernel\r\n
//! Server sends: <raw binary data><EOF>
//! ```

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "serve" => {
            if args.len() < 3 {
                eprintln!("Error: Missing kernel path");
                eprintln!("Usage: {} serve <kernel-path>", args[0]);
                std::process::exit(1);
            }
            serve_kernel(&args[2]);
        }

        "cmd" => {
            if args.len() < 4 {
                eprintln!("Error: Missing arguments");
                eprintln!("Usage: {} cmd <pi-addr:port> <command>", args[0]);
                std::process::exit(1);
            }
            let result = run_command(&args[2], &args[3..].join(" "));
            match result {
                Ok(output) => {
                    print!("{}", output);
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "help" | "--help" | "-h" => {
            print_usage();
            std::process::exit(0);
        }

        _ => {
            eprintln!("Error: Unknown command '{}'", args[1]);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("DaedalusOS Development Server");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    daedalus-dev-server serve <kernel-path>");
    eprintln!("    daedalus-dev-server cmd <pi-addr:port> <command>");
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!("    serve    Start HTTP server to serve kernel binary");
    eprintln!("    cmd      Run command on Pi via network shell");
    eprintln!("    help     Show this help message");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("    # Serve kernel on port 8000");
    eprintln!("    daedalus-dev-server serve target/aarch64-daedalus/release/kernel8.img");
    eprintln!();
    eprintln!("    # Run command on Pi");
    eprintln!("    daedalus-dev-server cmd 10.42.10.42:8080 meminfo");
}

/// Serve kernel binary over HTTP/0.9
///
/// Listens on 0.0.0.0:8000 and serves the kernel file to any client that
/// requests GET /kernel
fn serve_kernel(kernel_path: &str) {
    // Validate kernel file exists
    let path = Path::new(kernel_path);
    if !path.exists() {
        eprintln!("Error: Kernel file not found: {}", kernel_path);
        std::process::exit(1);
    }

    // Get file size for reporting
    let metadata = fs::metadata(path).expect("Failed to read file metadata");
    let file_size = metadata.len();

    println!("DaedalusOS Development Server");
    println!("==============================");
    println!();
    println!("Kernel file: {}", kernel_path);
    println!(
        "File size:   {} bytes ({:.2} KB)",
        file_size,
        file_size as f64 / 1024.0
    );
    println!();
    println!("Listening on: http://0.0.0.0:8000");
    println!("Endpoint:     GET /kernel");
    println!();
    println!("Waiting for connections... (Press Ctrl+C to stop)");
    println!();

    // Bind to all interfaces on port 8000
    let listener = TcpListener::bind("0.0.0.0:8000").expect("Failed to bind to port 8000");

    // Accept connections in loop
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Get peer address for logging
                let peer_addr = stream
                    .peer_addr()
                    .map(|addr| addr.to_string())
                    .unwrap_or_else(|_| "unknown".to_string());

                println!("[{}] Connection from {}", current_timestamp(), peer_addr);

                // Handle request
                if let Err(e) = handle_kernel_request(stream, kernel_path, file_size) {
                    eprintln!("[{}] Error serving kernel: {}", current_timestamp(), e);
                }
            }
            Err(e) => {
                eprintln!("[{}] Connection error: {}", current_timestamp(), e);
            }
        }
    }
}

/// Handle a single kernel request
fn handle_kernel_request(
    mut stream: TcpStream,
    kernel_path: &str,
    file_size: u64,
) -> std::io::Result<()> {
    let peer_addr = stream
        .peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    // Read request line
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    println!("[{}] Request: {}", current_timestamp(), request_line.trim());

    // Check if request is for kernel
    if !request_line.starts_with("GET /kernel") {
        eprintln!(
            "[{}] Invalid request (not GET /kernel)",
            current_timestamp()
        );
        return Ok(());
    }

    // Read kernel file
    let kernel_data = fs::read(kernel_path)?;

    // Verify size matches
    if kernel_data.len() != file_size as usize {
        eprintln!(
            "[{}] Warning: File size changed! Expected {} bytes, got {}",
            current_timestamp(),
            file_size,
            kernel_data.len()
        );
    }

    println!(
        "[{}] Sending {} bytes to {}",
        current_timestamp(),
        kernel_data.len(),
        peer_addr
    );

    // Send kernel data (HTTP/0.9 - no headers, just raw binary)
    stream.write_all(&kernel_data)?;
    stream.flush()?;

    println!("[{}] Transfer complete", current_timestamp());
    println!();

    Ok(())
}

/// Run command on Pi via network shell
///
/// Connects to Pi's network shell, sends command, reads output
fn run_command(pi_addr: &str, command: &str) -> std::io::Result<String> {
    println!("Connecting to {}...", pi_addr);

    let mut stream = TcpStream::connect(pi_addr)?;
    println!("Connected!");

    // Read prompt (discard)
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut prompt = String::new();
    reader.read_line(&mut prompt)?;

    println!("Sending command: {}", command);

    // Send command with newline
    stream.write_all(command.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    // Read output until next prompt
    let mut output = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;

        // EOF or next prompt
        if n == 0 || line.starts_with("daedalus>") {
            break;
        }

        output.push_str(&line);
    }

    Ok(output)
}

/// Get current timestamp string for logging
fn current_timestamp() -> String {
    use std::time::SystemTime;

    let now = SystemTime::now();
    let duration = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards");

    let secs = duration.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_format() {
        let ts = current_timestamp();
        assert_eq!(ts.len(), 8); // HH:MM:SS
        assert_eq!(ts.chars().nth(2), Some(':'));
        assert_eq!(ts.chars().nth(5), Some(':'));
    }
}

use crate::drivers::uart::WRITER;
use crate::{print, println};

const LINE_BUFFER_SIZE: usize = 256;
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shell command structure
pub struct Command<'a> {
    pub name: &'a str,
    pub args: &'a str,
}

impl<'a> Command<'a> {
    /// Parse a line into command and arguments
    pub fn parse(line: &'a str) -> Option<Self> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        let mut parts = line.splitn(2, ' ');
        let name = parts.next()?;
        let args = parts.next().unwrap_or("");

        Some(Command { name, args })
    }
}

/// Read a line from the UART with echo and basic line editing
fn read_line(buffer: &mut [u8]) -> usize {
    let mut pos = 0;

    loop {
        let ch = {
            let mut writer = WRITER.lock();
            writer.read_byte()
        };

        match ch {
            // Enter (CR or LF)
            b'\r' | b'\n' => {
                println!();
                return pos;
            }

            // Backspace or Delete
            b'\x7f' | b'\x08' => {
                if pos > 0 {
                    pos -= 1;
                    // Move cursor back, write space, move cursor back again
                    print!("\x08 \x08");
                }
            }

            // Ctrl-C
            b'\x03' => {
                println!("^C");
                return 0;
            }

            // Ctrl-U (clear line)
            b'\x15' => {
                while pos > 0 {
                    print!("\x08 \x08");
                    pos -= 1;
                }
            }

            // Printable ASCII
            0x20..=0x7e => {
                if pos < buffer.len() {
                    buffer[pos] = ch;
                    pos += 1;
                    // Echo the character
                    print!("{}", ch as char);
                }
            }

            // Ignore other control characters
            _ => {}
        }
    }
}

/// Execute a built-in command
fn execute_command(cmd: Command) {
    match cmd.name {
        "help" => {
            println!("DaedalusOS Shell Commands:");
            println!("  help      - Show this help message");
            println!("  echo      - Print arguments to console");
            println!("  clear     - Clear the screen");
            println!("  version   - Show kernel version");
            println!("  meminfo   - Display memory information (TODO)");
        }

        "echo" => {
            println!("{}", cmd.args);
        }

        "clear" => {
            // ANSI escape sequence to clear screen and move cursor to top
            print!("\x1b[2J\x1b[H");
        }

        "version" => {
            println!("DaedalusOS version {}", VERSION);
            println!("Target: Raspberry Pi 4 (AArch64)");
        }

        "meminfo" => {
            println!("Memory information:");
            println!("  Heap: Not yet implemented");
            println!("  TODO: Implement heap allocator to track memory usage");
        }

        _ => {
            println!("Unknown command: {}", cmd.name);
            println!("Type 'help' for available commands.");
        }
    }
}

/// Run the interactive shell REPL
pub fn run() -> ! {
    let mut line_buffer = [0u8; LINE_BUFFER_SIZE];

    println!();
    println!("Welcome to DaedalusOS!");
    println!("Type 'help' for available commands.");
    println!();

    loop {
        print!("daedalus> ");
        let len = read_line(&mut line_buffer);

        if len == 0 {
            continue;
        }

        // Convert buffer to str (safe because we only accept ASCII in read_line)
        if let Ok(line) = core::str::from_utf8(&line_buffer[..len]) {
            if let Some(cmd) = Command::parse(line) {
                execute_command(cmd);
            }
        }
    }
}

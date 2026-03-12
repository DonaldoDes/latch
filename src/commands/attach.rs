use anyhow::{Context, Result};
use std::io::{Read, Write};

use crate::server::protocol::{self, ClientMessage, ServerMessage};
use crate::session::resolve_session;

/// Run `latch attach <name|id>`
/// Connects to the session socket, receives history, then relays I/O in raw mode.
pub fn run(target: &str) -> Result<()> {
    let (id, _meta) = resolve_session(target)?;
    let paths = crate::session::SessionPaths::new(&id);

    if !paths.socket.exists() {
        anyhow::bail!("Session '{}' is not running (no socket found)", target);
    }

    // Connect to Unix socket
    let socket = std::os::unix::net::UnixStream::connect(&paths.socket)
        .context(format!("Cannot connect to session '{}'", target))?;

    let mut reader = socket.try_clone()?;
    let mut writer = socket;

    // Send Attach message
    let encoded = protocol::encode(&ClientMessage::Attach)?;
    writer.write_all(&encoded)?;
    writer.flush()?;

    // Read History message
    let history_msg = read_sync_message::<ServerMessage>(&mut reader)?;
    if let ServerMessage::History { data } = history_msg {
        std::io::stdout().write_all(&data)?;
        std::io::stdout().flush()?;
    }

    // Set terminal to raw mode
    let original_termios = set_raw_mode()?;

    // Relay: spawn a thread for stdin -> socket, main thread for socket -> stdout
    let mut writer_clone = writer.try_clone()?;
    let stdin_thread = std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buf = [0u8; 4096];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let msg = ClientMessage::Input {
                        data: buf[..n].to_vec(),
                    };
                    if let Ok(encoded) = protocol::encode(&msg) {
                        if writer_clone.write_all(&encoded).is_err() {
                            break;
                        }
                        let _ = writer_clone.flush();
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Read Output/SessionDead messages from server
    let mut stdout = std::io::stdout();
    loop {
        match read_sync_message::<ServerMessage>(&mut reader) {
            Ok(ServerMessage::Output { data }) => {
                stdout.write_all(&data)?;
                stdout.flush()?;
            }
            Ok(ServerMessage::SessionDead) => {
                eprintln!("\r\nSession ended.");
                break;
            }
            Ok(ServerMessage::History { .. }) => {
                // Unexpected second history message, ignore
            }
            Err(_) => {
                // Connection lost
                break;
            }
        }
    }

    // Restore terminal
    restore_termios(&original_termios);

    // Wait for stdin thread
    let _ = stdin_thread.join();

    Ok(())
}

/// Read a length-prefixed postcard message synchronously from a reader
fn read_sync_message<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > 16 * 1024 * 1024 {
        anyhow::bail!("Message too large: {} bytes", len);
    }

    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    protocol::decode(&payload)
}

/// Set terminal to raw mode, return the original termios for restoration
fn set_raw_mode() -> Result<libc::termios> {
    unsafe {
        let mut termios: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(libc::STDIN_FILENO, &mut termios) != 0 {
            anyhow::bail!("Failed to get terminal attributes");
        }
        let original = termios;

        libc::cfmakeraw(&mut termios);
        if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &termios) != 0 {
            anyhow::bail!("Failed to set raw mode");
        }

        Ok(original)
    }
}

/// Restore terminal to the given termios settings
fn restore_termios(termios: &libc::termios) {
    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, termios);
    }
}

pub mod protocol;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::sync::{broadcast, RwLock};

use crate::session::ring_buffer::RingBuffer;
use crate::session::{SessionMeta, SessionPaths, SessionStatus};
use protocol::{read_message, write_message, ClientMessage, ServerMessage};

/// Run the session server: PTY + IPC loop
/// This is called from the daemonised child process.
pub async fn run_server(paths: &SessionPaths, cmd: &str, session_name: &str) -> Result<()> {
    // Open PTY
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY")?;

    let mut cmd_builder = CommandBuilder::new(cmd);
    cmd_builder.env("LATCH_SESSION", session_name);

    let mut child = pair
        .slave
        .spawn_command(cmd_builder)
        .context("Failed to spawn command in PTY")?;

    // Drop slave end
    drop(pair.slave);

    // Get reader/writer for PTY master
    let mut pty_reader = pair.master.try_clone_reader().context("PTY clone reader")?;
    let pty_writer = Arc::new(std::sync::Mutex::new(
        pair.master.take_writer().context("PTY take writer")?,
    ));
    // Keep master alive so the PTY fd stays open; wrapped in Mutex for Sync
    let master = Arc::new(std::sync::Mutex::new(pair.master));

    // Ring buffer
    let ring_buffer = Arc::new(RwLock::new(RingBuffer::new(
        crate::session::ring_buffer::DEFAULT_CAPACITY,
    )));

    // Broadcast channel for PTY output -> all connected clients
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(256);

    // Start Unix socket listener
    let listener = UnixListener::bind(&paths.socket)
        .context(format!("Failed to bind socket: {:?}", paths.socket))?;

    // Task: read PTY output, push to ring buffer, broadcast to clients
    let tx_clone = tx.clone();
    let ring_clone = ring_buffer.clone();
    let history_path = paths.history.clone();
    let pty_read_task = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        let mut buf = [0u8; 4096];
        loop {
            match pty_reader.read(&mut buf) {
                Ok(0) => break, // EOF — child process exited
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    // Push to ring buffer
                    rt.block_on(async {
                        let mut rb = ring_clone.write().await;
                        rb.push(&data);
                        if let Err(e) = rb.save(&history_path) {
                            eprintln!("[latch] warning: failed to save ring buffer: {e}");
                        }
                    });
                    // Broadcast to connected clients (ignore if no receivers)
                    let _ = tx_clone.send(data);
                }
                Err(e) => {
                    // On macOS, EIO means the child process has exited
                    if e.kind() == std::io::ErrorKind::Other {
                        break;
                    }
                    eprintln!("PTY read error: {}", e);
                    break;
                }
            }
        }
    });

    // Task: accept client connections
    let accept_task = {
        let tx = tx.clone();
        let ring_buffer = ring_buffer.clone();
        let pty_writer = pty_writer.clone();
        let master_ref = master.clone();
        let meta_path = paths.meta.clone();
        let socket_path = paths.socket.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        let tx = tx.clone();
                        let ring_buffer = ring_buffer.clone();
                        let pty_writer = pty_writer.clone();
                        let master_ref = master_ref.clone();
                        let meta_path = meta_path.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_client(
                                stream,
                                tx,
                                ring_buffer,
                                pty_writer,
                                master_ref,
                                &meta_path,
                            )
                            .await
                            {
                                // Client disconnected — silently remove
                                let _ = e;
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                        break;
                    }
                }
            }
            // Cleanup socket
            let _ = std::fs::remove_file(&socket_path);
        })
    };

    // Wait for PTY read task to finish (child process exited)
    let _ = pty_read_task.await;

    // Child process has exited — wait for it
    let _ = child.wait();

    // Broadcast SessionDead to all connected clients
    let dead_msg = ServerMessage::SessionDead;
    if let Ok(encoded) = protocol::encode(&dead_msg) {
        let _ = tx.send(encoded);
    }

    // Update meta.json
    if let Err(e) = SessionMeta::update_status(&paths.meta, SessionStatus::Dead) {
        eprintln!("[latch] warning: failed to update session status to dead: {e}");
    }

    // Save final ring buffer
    {
        let rb = ring_buffer.read().await;
        if let Err(e) = rb.save(&paths.history) {
            eprintln!("[latch] warning: failed to save final ring buffer: {e}");
        }
    }

    // Give clients a moment to receive SessionDead
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Clean up
    accept_task.abort();
    let _ = std::fs::remove_file(&paths.socket);

    Ok(())
}

/// Handle a single client connection
async fn handle_client(
    stream: tokio::net::UnixStream,
    tx: broadcast::Sender<Vec<u8>>,
    ring_buffer: Arc<RwLock<RingBuffer>>,
    pty_writer: Arc<std::sync::Mutex<Box<dyn std::io::Write + Send>>>,
    master: Arc<std::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    meta_path: &Path,
) -> Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    // Read first message — must be Attach
    let msg: ClientMessage = read_message(&mut reader).await?;
    if msg != ClientMessage::Attach {
        anyhow::bail!("Expected Attach message, got {:?}", msg);
    }

    // Update meta.json to attached
    if let Err(e) = SessionMeta::update_status(meta_path, SessionStatus::Attached) {
        eprintln!("[latch] warning: failed to update session status to attached: {e}");
    }

    // Send history replay
    {
        let rb = ring_buffer.read().await;
        let history_data = rb.read_all();
        write_message(&mut writer, &ServerMessage::History { data: history_data }).await?;
    }

    // Subscribe to broadcast
    let mut rx = tx.subscribe();

    // Task: forward broadcast output to this client
    let writer = Arc::new(tokio::sync::Mutex::new(writer));
    let writer_clone = writer.clone();
    let output_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(data) => {
                    let mut w = writer_clone.lock().await;
                    // Check if this is an encoded ServerMessage (SessionDead broadcast)
                    // or raw PTY output
                    let msg = ServerMessage::Output { data };
                    if write_message(&mut *w, &msg).await.is_err() {
                        break; // Client disconnected
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("Client lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Read client messages
    loop {
        match read_message::<_, ClientMessage>(&mut reader).await {
            Ok(ClientMessage::Input { data }) => {
                if let Ok(mut w) = pty_writer.lock() {
                    let _ = w.write_all(&data);
                    let _ = w.flush();
                }
            }
            Ok(ClientMessage::Resize { cols, rows }) => {
                if let Ok(m) = master.lock() {
                    if let Err(e) = m.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    }) {
                        eprintln!("[latch] warning: failed to resize PTY: {e}");
                    }
                }
            }
            Ok(ClientMessage::Detach) => {
                break;
            }
            Ok(ClientMessage::Attach) => {
                // Already attached, ignore
            }
            Err(_) => {
                // Client disconnected
                break;
            }
        }
    }

    output_task.abort();
    // Yield to let the aborted task's receiver be dropped
    tokio::task::yield_now().await;

    // If no more subscribers, update to detached
    if tx.receiver_count() <= 1 {
        if let Err(e) = SessionMeta::update_status(meta_path, SessionStatus::Detached) {
            eprintln!("[latch] warning: failed to update session status to detached: {e}");
        }
    }

    Ok(())
}

use std::io::Write;

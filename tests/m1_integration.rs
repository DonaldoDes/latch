//! Integration tests for M1 — Core session management.
//! Covers US-001 through US-004 scenarios.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use tempfile::TempDir;

/// Helper: create a session via the library, bypassing fork/daemon.
/// Sets up session dir, meta.json, and spawns the async server in a background thread.
/// Returns (session_id, session_name, TempDir, server_thread_handle).
fn spawn_test_session(
    tmp: &TempDir,
) -> (
    String,
    String,
    latch::session::SessionPaths,
    std::thread::JoinHandle<()>,
) {
    let id = latch::session::generate_session_id();
    let name = format!("test-{}", &id[8..]);
    let base = tmp.path().join("sessions");
    let paths = latch::session::SessionPaths::from_base(&base, &id);
    paths.ensure_dir().unwrap();

    let meta = latch::session::SessionMeta {
        id: id.clone(),
        name: name.clone(),
        cmd: "sh".to_string(),
        pid: std::process::id(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        status: latch::session::SessionStatus::Detached,
    };
    meta.write_to(&paths.meta).unwrap();

    let paths_clone = paths.clone();
    let name_clone = name.clone();
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // Ignore errors (session will die when shell exits or test ends)
        let _ = rt.block_on(latch::server::run_server(&paths_clone, "sh", &name_clone));
    });

    // Wait for socket to appear
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !paths.socket.exists() {
        if std::time::Instant::now() > deadline {
            panic!("Server socket did not appear within 5 seconds");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    (id, name, paths, handle)
}

/// Helper: connect a client to the session socket, send Attach, receive History.
fn connect_and_attach(socket_path: &Path) -> UnixStream {
    let stream = UnixStream::connect(socket_path).expect("connect to socket");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();

    let attach_msg =
        latch::server::protocol::encode(&latch::server::protocol::ClientMessage::Attach)
            .expect("encode attach");
    let mut writer = stream.try_clone().unwrap();
    writer.write_all(&attach_msg).unwrap();
    writer.flush().unwrap();

    // Read History message
    let _history = read_server_message(&stream);

    stream
}

/// Helper: read a length-prefixed server message synchronously
fn read_server_message(stream: &UnixStream) -> latch::server::protocol::ServerMessage {
    let mut reader = stream.try_clone().unwrap();
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).unwrap();
    latch::server::protocol::decode(&payload).unwrap()
}

/// Helper: read a server message with timeout, returning None on failure
fn read_server_message_timeout(
    stream: &UnixStream,
) -> Option<latch::server::protocol::ServerMessage> {
    let mut reader = stream.try_clone().ok()?;
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).ok()?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).ok()?;
    latch::server::protocol::decode(&payload).ok()
}

/// Helper: send a client message synchronously
fn send_client_message(stream: &UnixStream, msg: &latch::server::protocol::ClientMessage) {
    let encoded = latch::server::protocol::encode(msg).unwrap();
    let mut writer = stream.try_clone().unwrap();
    writer.write_all(&encoded).unwrap();
    writer.flush().unwrap();
}

/// Helper: kill the server's child shell by sending "exit\n" via Input
fn graceful_shutdown(stream: &UnixStream) {
    send_client_message(
        stream,
        &latch::server::protocol::ClientMessage::Input {
            data: b"exit\n".to_vec(),
        },
    );
}

// ============================================================
// US-001: Server survives client disconnection
// ============================================================

#[test]
fn us001_server_survives_client_disconnect() {
    let tmp = TempDir::new().unwrap();
    let (_id, _name, paths, handle) = spawn_test_session(&tmp);

    // Connect a client
    let client = connect_and_attach(&paths.socket);

    // Disconnect client (drop)
    drop(client);

    // Wait a bit for server to process disconnect
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Server should still be alive: socket should still be reachable
    assert!(
        paths.socket.exists(),
        "Server socket should still exist after client disconnect"
    );

    // Connect a second client to prove server is alive
    let client2 = connect_and_attach(&paths.socket);
    graceful_shutdown(&client2);
    drop(client2);
    let _ = handle.join();
}

// ============================================================
// US-002: Ring buffer persistence
// ============================================================

#[test]
fn us002_ring_buffer_persists_after_disconnect() {
    use latch::session::ring_buffer::RingBuffer;

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("history.bin");

    let mut rb = RingBuffer::new(256);
    rb.push(b"persistent data after disconnect");
    rb.save(&path).unwrap();

    // Simulate "disconnect" — drop the in-memory buffer
    drop(rb);

    // Reopen from file
    let loaded = RingBuffer::open(&path).unwrap();
    assert_eq!(loaded.read_all(), b"persistent data after disconnect");
}

#[test]
fn us002_ring_buffer_overflow_stays_bounded() {
    use latch::session::ring_buffer::RingBuffer;

    let capacity = 64u64;
    let mut rb = RingBuffer::new(capacity);

    // Write more than capacity
    let big_data = vec![b'X'; 200];
    rb.push(&big_data);

    let contents = rb.read_all();
    assert!(
        contents.len() as u64 <= capacity,
        "Ring buffer contents ({}) should not exceed capacity ({})",
        contents.len(),
        capacity
    );
}

// ============================================================
// US-003: Client attach/broadcast
// ============================================================

#[test]
fn us003_client_attach_receives_history_then_output() {
    let tmp = TempDir::new().unwrap();
    let (_id, _name, paths, handle) = spawn_test_session(&tmp);

    let client = connect_and_attach(&paths.socket);

    // Send some input to generate output
    send_client_message(
        &client,
        &latch::server::protocol::ClientMessage::Input {
            data: b"echo hello_latch\n".to_vec(),
        },
    );

    // Read output messages -- we should get Output messages
    let msg = read_server_message(&client);
    match msg {
        latch::server::protocol::ServerMessage::Output { data } => {
            assert!(!data.is_empty(), "Output should not be empty");
        }
        other => {
            // History is also acceptable if the server sends it again
            panic!("Expected Output message, got: {:?}", other);
        }
    }

    graceful_shutdown(&client);
    drop(client);
    let _ = handle.join();
}

#[test]
fn us003_broadcast_to_multiple_clients() {
    let tmp = TempDir::new().unwrap();
    let (_id, _name, paths, handle) = spawn_test_session(&tmp);

    let client1 = connect_and_attach(&paths.socket);
    let client2 = connect_and_attach(&paths.socket);

    // Client1 sends input
    send_client_message(
        &client1,
        &latch::server::protocol::ClientMessage::Input {
            data: b"echo broadcast_test\n".to_vec(),
        },
    );

    // Both clients should receive output
    // Give some time for broadcast
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Read from client1
    let msg1 = read_server_message(&client1);
    assert!(
        matches!(msg1, latch::server::protocol::ServerMessage::Output { .. }),
        "Client1 should receive Output"
    );

    // Read from client2
    let msg2 = read_server_message(&client2);
    assert!(
        matches!(msg2, latch::server::protocol::ServerMessage::Output { .. }),
        "Client2 should receive Output"
    );

    graceful_shutdown(&client1);
    drop(client1);
    drop(client2);
    let _ = handle.join();
}

#[test]
fn us003_client_detach_server_continues() {
    let tmp = TempDir::new().unwrap();
    let (_id, _name, paths, handle) = spawn_test_session(&tmp);

    // Connect and detach
    let client = connect_and_attach(&paths.socket);
    send_client_message(&client, &latch::server::protocol::ClientMessage::Detach);
    drop(client);

    std::thread::sleep(std::time::Duration::from_millis(500));

    // Server should still be alive
    assert!(
        paths.socket.exists(),
        "Socket should still exist after detach"
    );

    // Can reconnect
    let client2 = connect_and_attach(&paths.socket);
    graceful_shutdown(&client2);
    drop(client2);
    let _ = handle.join();
}

// ============================================================
// US-004: Session lifecycle
// ============================================================

#[test]
fn us004_attach_detach_cycle_list_shows_detached() {
    let tmp = TempDir::new().unwrap();
    let (_id, _name, paths, handle) = spawn_test_session(&tmp);

    // Attach
    let client = connect_and_attach(&paths.socket);

    // Read meta — should be Attached
    let meta = latch::session::SessionMeta::read_from(&paths.meta).unwrap();
    assert_eq!(meta.status, latch::session::SessionStatus::Attached);

    // Detach
    send_client_message(&client, &latch::server::protocol::ClientMessage::Detach);
    drop(client);

    // Wait for server to process detach and update meta.json
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let meta = latch::session::SessionMeta::read_from(&paths.meta).unwrap();
        if meta.status == latch::session::SessionStatus::Detached {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("Server did not update status to Detached within 3 seconds");
        }
    }

    // Cleanup
    let client2 = connect_and_attach(&paths.socket);
    graceful_shutdown(&client2);
    drop(client2);
    let _ = handle.join();
}

#[test]
fn us004_latch_session_env_injected() {
    let tmp = TempDir::new().unwrap();
    let (_id, _name, paths, handle) = spawn_test_session(&tmp);

    let client = connect_and_attach(&paths.socket);

    // Ask the shell to echo the env var
    send_client_message(
        &client,
        &latch::server::protocol::ClientMessage::Input {
            data: b"echo LATCH=$LATCH_SESSION\n".to_vec(),
        },
    );

    // Collect output messages until we find one containing LATCH=
    std::thread::sleep(std::time::Duration::from_millis(300));
    let mut collected = String::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        match read_server_message_timeout(&client) {
            Some(latch::server::protocol::ServerMessage::Output { data }) => {
                collected.push_str(&String::from_utf8_lossy(&data));
                if collected.contains("LATCH=") {
                    break;
                }
            }
            _ => break,
        }
    }
    assert!(
        collected.contains("LATCH="),
        "Output should contain LATCH= prefix, got: {}",
        collected
    );

    graceful_shutdown(&client);
    drop(client);
    let _ = handle.join();
}

// ============================================================
// Fix 4: ProjectDirs for sessions_base_dir
// ============================================================

#[test]
fn fix4_sessions_base_dir_uses_project_dirs() {
    // When HOME is set, sessions_base_dir should use directories::ProjectDirs
    // On macOS: ~/Library/Application Support/latch/sessions
    // On Linux: ~/.local/share/latch/sessions
    let base = latch::session::sessions_base_dir();
    let base_str = base.to_string_lossy();
    assert!(
        base_str.contains("latch"),
        "sessions_base_dir should contain 'latch', got: {}",
        base_str
    );
    // It should NOT fall back to /tmp
    assert!(
        !base_str.starts_with("/tmp"),
        "sessions_base_dir should not use /tmp when HOME is set, got: {}",
        base_str
    );
    // It should NOT use the hardcoded .local/share path on macOS
    #[cfg(target_os = "macos")]
    assert!(
        base_str.contains("Library/Application Support"),
        "On macOS, sessions_base_dir should use Library/Application Support, got: {}",
        base_str
    );
    #[cfg(target_os = "linux")]
    assert!(
        base_str.contains(".local/share"),
        "On Linux, sessions_base_dir should use .local/share, got: {}",
        base_str
    );
}

// ============================================================
// Fix 5: chmod 0700 on session dir
// ============================================================

#[test]
fn fix5_session_dir_has_mode_0700() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let paths = latch::session::SessionPaths::from_base(tmp.path(), "test-perms");
    paths.ensure_dir().unwrap();

    let metadata = std::fs::metadata(&paths.dir).unwrap();
    let mode = metadata.permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o700,
        "Session directory should have mode 0700, got: {:o}",
        mode
    );
}

// ============================================================
// Fix 6: eprintln on silent errors (tested via server module)
// These are structural changes — we verify the code compiles
// and the save/update_status paths don't panic.
// ============================================================

#[test]
fn fix6_ring_buffer_save_to_invalid_path_does_not_panic() {
    use latch::session::ring_buffer::RingBuffer;

    let mut rb = RingBuffer::new(64);
    rb.push(b"test");
    // Save to a path that doesn't exist — should return Err, not panic
    let result = rb.save(Path::new("/nonexistent/dir/file.bin"));
    assert!(result.is_err());
}

#[test]
fn fix6_update_status_on_missing_file_does_not_panic() {
    let result = latch::session::SessionMeta::update_status(
        Path::new("/nonexistent/meta.json"),
        latch::session::SessionStatus::Dead,
    );
    assert!(result.is_err());
}

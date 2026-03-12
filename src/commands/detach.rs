use anyhow::Result;

use crate::server::protocol::{encode, ClientMessage};
use crate::session::resolve_session;

/// Resolve the target session for detach: argument > $LATCH_SESSION
pub fn resolve_detach_target(session: Option<String>) -> Result<String> {
    if let Some(s) = session {
        return Ok(s);
    }

    match std::env::var("LATCH_SESSION") {
        Ok(s) if !s.is_empty() => Ok(s),
        _ => anyhow::bail!("error: not in a latch session (LATCH_SESSION not set)"),
    }
}

/// Run `latch detach [name|id]`
pub fn run(session: Option<String>) -> Result<()> {
    let target = resolve_detach_target(session)?;
    let (id, _meta) = resolve_session(&target)?;
    let paths = crate::session::SessionPaths::new(&id);

    // Connect to socket and send Detach
    let socket = std::os::unix::net::UnixStream::connect(&paths.socket)
        .map_err(|_| anyhow::anyhow!("Cannot connect to session '{}' — is it running?", target))?;

    let mut writer = std::io::BufWriter::new(&socket);
    let encoded = encode(&ClientMessage::Detach)?;
    std::io::Write::write_all(&mut writer, &encoded)?;
    std::io::Write::flush(&mut writer)?;

    println!("Detached from session '{}'", target);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_detach_target_with_argument() {
        let target = resolve_detach_target(Some("mysession".to_string())).unwrap();
        assert_eq!(target, "mysession");
    }

    #[test]
    #[serial_test::serial]
    fn resolve_detach_target_from_env() {
        std::env::set_var("LATCH_SESSION", "envsession");
        let target = resolve_detach_target(None).unwrap();
        assert_eq!(target, "envsession");
        std::env::remove_var("LATCH_SESSION");
    }

    #[test]
    #[serial_test::serial]
    fn resolve_detach_target_error_when_no_env() {
        std::env::remove_var("LATCH_SESSION");
        let result = resolve_detach_target(None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert_eq!(err, "error: not in a latch session (LATCH_SESSION not set)");
    }

    #[test]
    #[serial_test::serial]
    fn resolve_detach_argument_takes_priority_over_env() {
        std::env::set_var("LATCH_SESSION", "envsession");
        let target = resolve_detach_target(Some("explicit".to_string())).unwrap();
        assert_eq!(target, "explicit");
        std::env::remove_var("LATCH_SESSION");
    }
}

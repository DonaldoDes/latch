use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Messages sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    Attach,
    Input { data: Vec<u8> },
    Resize { cols: u16, rows: u16 },
    Detach,
}

/// Messages sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    Output { data: Vec<u8> },
    History { data: Vec<u8> },
    SessionDead,
}

/// Encode a message as length-prefixed postcard bytes: [u32 BE length][postcard payload]
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    let payload = postcard::to_allocvec(msg).context("Failed to serialize message")?;
    let len = payload.len() as u32;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Decode a postcard payload (without length prefix) into a message
pub fn decode<'a, T: Deserialize<'a>>(data: &'a [u8]) -> Result<T> {
    postcard::from_bytes(data).context("Failed to deserialize message")
}

/// Read a length-prefixed message from an async reader
pub async fn read_message<R: AsyncReadExt + Unpin, T: for<'a> Deserialize<'a>>(
    reader: &mut R,
) -> Result<T> {
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .await
        .context("Failed to read message length")?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > 16 * 1024 * 1024 {
        anyhow::bail!("Message too large: {} bytes", len);
    }

    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .await
        .context("Failed to read message payload")?;

    decode(&payload)
}

/// Write a length-prefixed message to an async writer
pub async fn write_message<W: AsyncWriteExt + Unpin, T: Serialize>(
    writer: &mut W,
    msg: &T,
) -> Result<()> {
    let encoded = encode(msg)?;
    writer
        .write_all(&encoded)
        .await
        .context("Failed to write message")?;
    writer.flush().await.context("Failed to flush writer")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_client_attach() {
        let msg = ClientMessage::Attach;
        let encoded = encode(&msg).unwrap();
        // First 4 bytes are length (big-endian)
        let len = u32::from_be_bytes(encoded[..4].try_into().unwrap()) as usize;
        assert_eq!(len, encoded.len() - 4);
        let decoded: ClientMessage = decode(&encoded[4..]).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn encode_decode_client_input() {
        let msg = ClientMessage::Input {
            data: b"hello".to_vec(),
        };
        let encoded = encode(&msg).unwrap();
        let decoded: ClientMessage = decode(&encoded[4..]).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn encode_decode_client_resize() {
        let msg = ClientMessage::Resize {
            cols: 120,
            rows: 40,
        };
        let encoded = encode(&msg).unwrap();
        let decoded: ClientMessage = decode(&encoded[4..]).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn encode_decode_client_detach() {
        let msg = ClientMessage::Detach;
        let encoded = encode(&msg).unwrap();
        let decoded: ClientMessage = decode(&encoded[4..]).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn encode_decode_server_output() {
        let msg = ServerMessage::Output {
            data: b"ls output".to_vec(),
        };
        let encoded = encode(&msg).unwrap();
        let decoded: ServerMessage = decode(&encoded[4..]).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn encode_decode_server_history() {
        let msg = ServerMessage::History {
            data: vec![1, 2, 3, 4, 5],
        };
        let encoded = encode(&msg).unwrap();
        let decoded: ServerMessage = decode(&encoded[4..]).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn encode_decode_server_session_dead() {
        let msg = ServerMessage::SessionDead;
        let encoded = encode(&msg).unwrap();
        let decoded: ServerMessage = decode(&encoded[4..]).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn length_prefix_is_big_endian() {
        let msg = ClientMessage::Attach;
        let encoded = encode(&msg).unwrap();
        let len_bytes = &encoded[..4];
        let len = u32::from_be_bytes(len_bytes.try_into().unwrap());
        assert!(len > 0);
        assert_eq!(len as usize + 4, encoded.len());
    }

    #[tokio::test]
    async fn async_write_read_roundtrip() {
        let msg = ServerMessage::Output {
            data: b"async test data".to_vec(),
        };

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded: ServerMessage = read_message(&mut cursor).await.unwrap();
        assert_eq!(decoded, msg);
    }

    #[tokio::test]
    async fn async_multiple_messages_in_sequence() {
        let msgs = vec![
            ServerMessage::History {
                data: b"history".to_vec(),
            },
            ServerMessage::Output {
                data: b"output1".to_vec(),
            },
            ServerMessage::Output {
                data: b"output2".to_vec(),
            },
        ];

        let mut buf = Vec::new();
        for msg in &msgs {
            write_message(&mut buf, msg).await.unwrap();
        }

        let mut cursor = std::io::Cursor::new(buf);
        for expected in &msgs {
            let decoded: ServerMessage = read_message(&mut cursor).await.unwrap();
            assert_eq!(&decoded, expected);
        }
    }
}

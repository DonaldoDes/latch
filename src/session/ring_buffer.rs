use anyhow::{Context, Result};
use std::path::Path;

/// Magic bytes for the ring buffer file header
pub const MAGIC: &[u8; 4] = b"LTCH";
/// Ring buffer format version
pub const VERSION: u32 = 1;
/// Header size: 4 (magic) + 4 (version) + 8 (write_pos) + 8 (capacity) = 24 bytes
pub const HEADER_SIZE: usize = 24;
/// Default capacity: 1 MB
pub const DEFAULT_CAPACITY: u64 = 1_048_576;

/// A persistent ring buffer backed by an in-memory Vec.
/// Data wraps around when capacity is reached.
pub struct RingBuffer {
    capacity: u64,
    write_pos: u64,
    data: Vec<u8>,
}

impl RingBuffer {
    /// Create a new empty ring buffer with the given capacity
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            write_pos: 0,
            data: vec![0u8; capacity as usize],
        }
    }

    /// Open an existing ring buffer from a file
    pub fn open(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read ring buffer: {:?}", path))?;

        if bytes.len() < HEADER_SIZE {
            anyhow::bail!("Ring buffer file too small: {} bytes", bytes.len());
        }

        // Validate magic
        if &bytes[0..4] != MAGIC {
            anyhow::bail!("Invalid ring buffer magic");
        }

        // Read version
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != VERSION {
            anyhow::bail!("Unsupported ring buffer version: {}", version);
        }

        // Read write_pos and capacity
        let write_pos = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let capacity = u64::from_le_bytes(bytes[16..24].try_into().unwrap());

        let data = bytes[HEADER_SIZE..].to_vec();
        if data.len() != capacity as usize {
            anyhow::bail!(
                "Ring buffer data size mismatch: expected {}, got {}",
                capacity,
                data.len()
            );
        }

        Ok(Self {
            capacity,
            write_pos,
            data,
        })
    }

    /// Write data into the ring buffer (wraps around)
    pub fn push(&mut self, data: &[u8]) {
        for &byte in data {
            let pos = (self.write_pos % self.capacity) as usize;
            self.data[pos] = byte;
            self.write_pos += 1;
        }
    }

    /// Read all data in chronological order
    pub fn read_all(&self) -> Vec<u8> {
        if self.write_pos == 0 {
            return Vec::new();
        }

        if self.write_pos <= self.capacity {
            // Buffer hasn't wrapped yet — return data from start to write_pos
            return self.data[..self.write_pos as usize].to_vec();
        }

        // Buffer has wrapped — oldest data starts at write_pos % capacity
        let start = (self.write_pos % self.capacity) as usize;
        let mut result = Vec::with_capacity(self.capacity as usize);
        result.extend_from_slice(&self.data[start..]);
        result.extend_from_slice(&self.data[..start]);
        result
    }

    /// Serialize the ring buffer to bytes (header + data)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE + self.capacity as usize);
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&VERSION.to_le_bytes());
        buf.extend_from_slice(&self.write_pos.to_le_bytes());
        buf.extend_from_slice(&self.capacity.to_le_bytes());
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Save the ring buffer to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_bytes())
            .with_context(|| format!("Failed to write ring buffer: {:?}", path))
    }

    /// Get the capacity
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get the total bytes written (may exceed capacity if wrapped)
    pub fn write_pos(&self) -> u64 {
        self.write_pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_buffer_has_correct_capacity() {
        let buf = RingBuffer::new(1024);
        assert_eq!(buf.capacity(), 1024);
        assert_eq!(buf.write_pos(), 0);
    }

    #[test]
    fn new_buffer_read_all_returns_empty() {
        let buf = RingBuffer::new(1024);
        assert!(buf.read_all().is_empty());
    }

    #[test]
    fn push_and_read_small_data() {
        let mut buf = RingBuffer::new(1024);
        buf.push(b"hello world");
        let data = buf.read_all();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn push_preserves_chronological_order() {
        let mut buf = RingBuffer::new(1024);
        buf.push(b"line1\n");
        buf.push(b"line2\n");
        buf.push(b"line3\n");
        let data = buf.read_all();
        assert_eq!(data, b"line1\nline2\nline3\n");
    }

    #[test]
    fn ring_buffer_wraps_on_overflow() {
        let mut buf = RingBuffer::new(10);
        // Write 15 bytes into a 10-byte buffer
        buf.push(b"0123456789ABCDE");
        let data = buf.read_all();
        // Should contain the last 10 bytes
        assert_eq!(data, b"56789ABCDE");
        assert_eq!(data.len(), 10);
    }

    #[test]
    fn ring_buffer_file_size_stays_bounded() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("history.bin");
        let mut buf = RingBuffer::new(100);
        // Write more than capacity
        buf.push(&vec![b'X'; 200]);
        buf.save(&path).unwrap();
        let file_size = std::fs::metadata(&path).unwrap().len();
        // File = header (24) + capacity (100) = 124 bytes
        assert_eq!(file_size, HEADER_SIZE as u64 + 100);
    }

    #[test]
    fn save_and_open_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.bin");

        let mut buf = RingBuffer::new(256);
        buf.push(b"persistent data");
        buf.save(&path).unwrap();

        let loaded = RingBuffer::open(&path).unwrap();
        assert_eq!(loaded.read_all(), b"persistent data");
        assert_eq!(loaded.capacity(), 256);
        assert_eq!(loaded.write_pos(), 15);
    }

    #[test]
    fn open_validates_magic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.bin");
        std::fs::write(&path, b"BADHEADERxxxxxxxxxxxxxxxx").unwrap();
        assert!(RingBuffer::open(&path).is_err());
    }

    #[test]
    fn open_validates_version() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("badver.bin");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&99u32.to_le_bytes()); // bad version
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(RingBuffer::open(&path).is_err());
    }

    #[test]
    fn header_contains_correct_magic_and_version() {
        let buf = RingBuffer::new(64);
        let bytes = buf.to_bytes();
        assert_eq!(&bytes[0..4], MAGIC);
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), VERSION);
    }

    #[test]
    fn default_capacity_is_one_mb() {
        assert_eq!(DEFAULT_CAPACITY, 1_048_576);
    }

    #[test]
    fn wrapped_buffer_roundtrip_through_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("wrap.bin");

        let mut buf = RingBuffer::new(10);
        buf.push(b"0123456789ABCDE"); // 15 bytes into 10
        buf.save(&path).unwrap();

        let loaded = RingBuffer::open(&path).unwrap();
        assert_eq!(loaded.read_all(), b"56789ABCDE");
    }
}

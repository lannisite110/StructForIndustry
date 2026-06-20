//! Fixed-size HAL → bus notification (see `core/contracts/abi/hal_ipc.h`).

pub const MAGIC: u32 = 0x0049_4653;
pub const VERSION: u16 = 1;
pub const NOTIFY_SIZE: usize = 144;

pub const SOURCE_ID_LEN: usize = 32;
pub const POOL_ID_LEN: usize = 16;
pub const SHM_NAME_LEN: usize = 32;

const OFF_MAGIC: usize = 0;
const OFF_VERSION: usize = 4;
const OFF_FRAME_ID: usize = 8;
const OFF_TIMESTAMP: usize = 16;
const OFF_SEQUENCE: usize = 24;
const OFF_WIDTH: usize = 32;
const OFF_HEIGHT: usize = 36;
const OFF_STRIDE: usize = 40;
const OFF_FORMAT: usize = 44;
const OFF_SOURCE_ID: usize = 48;
const OFF_POOL_ID: usize = 80;
const OFF_SLOT: usize = 96;
const OFF_GENERATION: usize = 100;
const OFF_BYTE_LENGTH: usize = 104;
const OFF_SHM_NAME: usize = 112;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalFrameNotify {
    pub frame_id: u64,
    pub timestamp_ns: u64,
    pub sequence: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: u8,
    pub source_id: [u8; SOURCE_ID_LEN],
    pub pool_id: [u8; POOL_ID_LEN],
    pub slot_index: u32,
    pub generation: u32,
    pub byte_length: u64,
    pub shm_name: [u8; SHM_NAME_LEN],
}

impl HalFrameNotify {
    pub fn decode(bytes: &[u8]) -> Result<Self, HalIpcError> {
        if bytes.len() != NOTIFY_SIZE {
            return Err(HalIpcError::InvalidLength(bytes.len()));
        }
        let magic = read_u32(bytes, OFF_MAGIC);
        let version = read_u16(bytes, OFF_VERSION);
        if magic != MAGIC {
            return Err(HalIpcError::BadMagic(magic));
        }
        if version != VERSION {
            return Err(HalIpcError::BadVersion(version));
        }

        let mut source_id = [0u8; SOURCE_ID_LEN];
        source_id.copy_from_slice(&bytes[OFF_SOURCE_ID..OFF_SOURCE_ID + SOURCE_ID_LEN]);
        let mut pool_id = [0u8; POOL_ID_LEN];
        pool_id.copy_from_slice(&bytes[OFF_POOL_ID..OFF_POOL_ID + POOL_ID_LEN]);
        let mut shm_name = [0u8; SHM_NAME_LEN];
        shm_name.copy_from_slice(&bytes[OFF_SHM_NAME..OFF_SHM_NAME + SHM_NAME_LEN]);

        Ok(Self {
            frame_id: read_u64(bytes, OFF_FRAME_ID),
            timestamp_ns: read_u64(bytes, OFF_TIMESTAMP),
            sequence: read_u64(bytes, OFF_SEQUENCE),
            width: read_u32(bytes, OFF_WIDTH),
            height: read_u32(bytes, OFF_HEIGHT),
            stride: read_u32(bytes, OFF_STRIDE),
            format: bytes[OFF_FORMAT],
            source_id,
            pool_id,
            slot_index: read_u32(bytes, OFF_SLOT),
            generation: read_u32(bytes, OFF_GENERATION),
            byte_length: read_u64(bytes, OFF_BYTE_LENGTH),
            shm_name,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![0u8; NOTIFY_SIZE];
        write_u32(&mut out, OFF_MAGIC, MAGIC);
        write_u16(&mut out, OFF_VERSION, VERSION);
        write_u64(&mut out, OFF_FRAME_ID, self.frame_id);
        write_u64(&mut out, OFF_TIMESTAMP, self.timestamp_ns);
        write_u64(&mut out, OFF_SEQUENCE, self.sequence);
        write_u32(&mut out, OFF_WIDTH, self.width);
        write_u32(&mut out, OFF_HEIGHT, self.height);
        write_u32(&mut out, OFF_STRIDE, self.stride);
        out[OFF_FORMAT] = self.format;
        out[OFF_SOURCE_ID..OFF_SOURCE_ID + SOURCE_ID_LEN].copy_from_slice(&self.source_id);
        out[OFF_POOL_ID..OFF_POOL_ID + POOL_ID_LEN].copy_from_slice(&self.pool_id);
        write_u32(&mut out, OFF_SLOT, self.slot_index);
        write_u32(&mut out, OFF_GENERATION, self.generation);
        write_u64(&mut out, OFF_BYTE_LENGTH, self.byte_length);
        out[OFF_SHM_NAME..OFF_SHM_NAME + SHM_NAME_LEN].copy_from_slice(&self.shm_name);
        out
    }

    pub fn source_id_str(&self) -> &str {
        cstr_prefix(&self.source_id)
    }

    pub fn pool_id_str(&self) -> &str {
        cstr_prefix(&self.pool_id)
    }

    pub fn shm_name_str(&self) -> &str {
        cstr_prefix(&self.shm_name)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HalIpcError {
    #[error("invalid notify length {0}, expected {NOTIFY_SIZE}")]
    InvalidLength(usize),
    #[error("bad magic {0:#010x}")]
    BadMagic(u32),
    #[error("unsupported ipc version {0}")]
    BadVersion(u16),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

fn read_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(buf[off..off + 2].try_into().unwrap())
}

fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

fn read_u64(buf: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(buf[off..off + 8].try_into().unwrap())
}

fn write_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn write_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn write_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn cstr_prefix(bytes: &[u8]) -> &str {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end]).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encode_decode() {
        let mut notify = HalFrameNotify {
            frame_id: 9,
            timestamp_ns: 123,
            sequence: 1,
            width: 64,
            height: 48,
            stride: 64,
            format: 1,
            source_id: [0; SOURCE_ID_LEN],
            pool_id: [0; POOL_ID_LEN],
            slot_index: 0,
            generation: 1,
            byte_length: 64 * 48,
            shm_name: [0; SHM_NAME_LEN],
        };
        copy_str(&mut notify.source_id, "synthetic-0");
        copy_str(&mut notify.pool_id, "hal.default");
        copy_str(&mut notify.shm_name, "/sfi.pool.0");

        let bytes = notify.encode();
        assert_eq!(bytes.len(), NOTIFY_SIZE);
        let decoded = HalFrameNotify::decode(&bytes).unwrap();
        assert_eq!(decoded.frame_id, 9);
        assert_eq!(decoded.source_id_str(), "synthetic-0");
    }

    fn copy_str(dst: &mut [u8], s: &str) {
        let b = s.as_bytes();
        dst[..b.len()].copy_from_slice(b);
    }
}

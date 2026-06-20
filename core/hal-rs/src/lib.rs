//! Synthetic HAL capture (Phase 1), in Rust.
//!
//! Replaces the former Zig `sfi-capture`: fills a Gray8 POSIX shm pool with a
//! test pattern and publishes `HalFrameNotify` frames to core-bus over the HAL
//! Unix socket. Same wire format (`HalFrameNotify`) as the production line
//! publishers — this is the minimal "camera stand-in" for the bus.

use sfi_core_bus::{HalFrameNotify, POOL_ID_LEN, SHM_NAME_LEN, SOURCE_ID_LEN};

pub const WIDTH: u32 = 640;
pub const HEIGHT: u32 = 480;
pub const STRIDE: u32 = 640;
pub const SHM_NAME: &str = "/sfi.pool.0";
pub const SOURCE_ID: &str = "synthetic-0";
pub const POOL_ID: &str = "hal.default";

pub fn byte_length() -> u64 {
    (STRIDE * HEIGHT) as u64
}

fn copy_str(dst: &mut [u8], s: &str) {
    let n = dst.len().min(s.len());
    dst[..n].copy_from_slice(&s.as_bytes()[..n]);
}

/// Build a HAL notify for a synthetic frame in a 2-slot pool.
pub fn build_notify(frame_id: u64, timestamp_ns: u64, slot_index: u32) -> HalFrameNotify {
    let mut notify = HalFrameNotify {
        frame_id: frame_id + 1,
        timestamp_ns,
        sequence: frame_id,
        width: WIDTH,
        height: HEIGHT,
        stride: STRIDE,
        format: 1,
        source_id: [0; SOURCE_ID_LEN],
        pool_id: [0; POOL_ID_LEN],
        slot_index,
        generation: 1,
        byte_length: byte_length(),
        shm_name: [0; SHM_NAME_LEN],
    };
    copy_str(&mut notify.source_id, SOURCE_ID);
    copy_str(&mut notify.pool_id, POOL_ID);
    copy_str(&mut notify.shm_name, SHM_NAME);
    notify
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_has_expected_layout() {
        let n = build_notify(0, 42, 0);
        assert_eq!(n.frame_id, 1);
        assert_eq!(n.sequence, 0);
        assert_eq!(n.width, WIDTH);
        assert_eq!(n.byte_length, byte_length());
        assert_eq!(n.shm_name_str(), SHM_NAME);
        assert_eq!(n.source_id_str(), SOURCE_ID);
    }

    #[test]
    fn notify_roundtrips_through_wire() {
        let n = build_notify(7, 99, 1);
        let decoded = HalFrameNotify::decode(&n.encode()).unwrap();
        assert_eq!(decoded.frame_id, 8);
        assert_eq!(decoded.slot_index, 1);
        assert_eq!(decoded.pool_id_str(), POOL_ID);
    }
}

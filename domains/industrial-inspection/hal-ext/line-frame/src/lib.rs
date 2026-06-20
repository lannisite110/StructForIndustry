//! Gray8 frame pool helpers for line-publisher and plc-trigger.

use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use memmap2::{MmapMut, MmapOptions};
use sfi_core_bus::{HalFrameNotify, POOL_ID_LEN, SHM_NAME_LEN, SOURCE_ID_LEN};

pub const WIDTH: u32 = 64;
pub const HEIGHT: u32 = 48;
pub const STRIDE: u32 = 64;

pub fn byte_length() -> usize {
    (STRIDE * HEIGHT) as usize
}

pub fn shm_file_path(name: &str) -> PathBuf {
    if name.starts_with("/dev/shm/") {
        PathBuf::from(name)
    } else if name.starts_with('/') {
        PathBuf::from(format!("/dev/shm/{}", &name[1..]))
    } else {
        PathBuf::from(format!("/dev/shm/{name}"))
    }
}

pub fn shm_notify_name(name: &str) -> String {
    if name.starts_with('/') {
        name.to_string()
    } else {
        format!("/{name}")
    }
}

pub fn fill_frame(buf: &mut [u8], frame_index: u64, inject_defect: bool) {
    for (i, px) in buf.iter_mut().enumerate() {
        let x = (i as u32) % STRIDE;
        let y = (i as u32) / STRIDE;
        *px = ((x + y + frame_index as u32) % 256) as u8;
    }
    if inject_defect {
        let cx = (STRIDE / 2) as usize;
        let cy = (HEIGHT / 2) as usize;
        for dy in 0..8 {
            for dx in 0..8 {
                let x = cx + dx;
                let y = cy + dy;
                if x < STRIDE as usize && y < HEIGHT as usize {
                    buf[y * STRIDE as usize + x] = 250;
                }
            }
        }
    }
}

pub fn map_shm(name: &str, size: usize) -> io::Result<MmapMut> {
    let path = shm_file_path(name);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;
    file.set_len(size as u64)?;
    unsafe { MmapOptions::new().len(size).map_mut(&file) }
}

pub fn build_notify(frame_id: u64, shm_name: &str, byte_len: u64) -> HalFrameNotify {
    let mut notify = HalFrameNotify {
        frame_id,
        timestamp_ns: now_ns(),
        sequence: frame_id,
        width: WIDTH,
        height: HEIGHT,
        stride: STRIDE,
        format: 1,
        source_id: [0; SOURCE_ID_LEN],
        pool_id: [0; POOL_ID_LEN],
        slot_index: 0,
        generation: 1,
        byte_length: byte_len,
        shm_name: [0; SHM_NAME_LEN],
    };
    copy_str(&mut notify.source_id, "line-trigger-0");
    copy_str(&mut notify.pool_id, "hal.line");
    copy_str(&mut notify.shm_name, &shm_notify_name(shm_name));
    notify
}

fn copy_str(dst: &mut [u8], s: &str) {
    let n = dst.len().min(s.len());
    dst[..n].copy_from_slice(&s.as_bytes()[..n]);
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_shm_name_matches_julia_resolver() {
        assert_eq!(shm_notify_name("sfi.aoi.demo"), "/sfi.aoi.demo");
        assert_eq!(
            shm_file_path("sfi.aoi.demo"),
            PathBuf::from("/dev/shm/sfi.aoi.demo")
        );
    }
}

//! POSIX shm Gray8 helpers for out-of-process plugins.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Resolve HAL notify shm name to a filesystem path under `/dev/shm`.
///
/// HAL notify uses `/sfi.aoi.line.0` (single segment). Absolute paths with
/// multiple segments (e.g. temp files in tests) are used as-is.
pub fn resolve_shm_path(shm_name: &str) -> PathBuf {
    if shm_name.starts_with("/dev/shm/") {
        PathBuf::from(shm_name)
    } else if shm_name.starts_with('/') && shm_name[1..].contains('/') {
        PathBuf::from(shm_name)
    } else if shm_name.starts_with('/') {
        PathBuf::from(format!("/dev/shm/{}", &shm_name[1..]))
    } else {
        PathBuf::from(format!("/dev/shm/{shm_name}"))
    }
}

pub fn read_gray8(shm_name: &str, byte_length: u64, offset: u64) -> io::Result<Vec<u8>> {
    let path = resolve_shm_path(shm_name);
    let mut file = File::open(path)?;
    if offset > 0 {
        let mut discard = vec![0u8; offset as usize];
        file.read_exact(&mut discard)?;
    }
    let mut buf = vec![0u8; byte_length as usize];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn gray_mean(pixels: &[u8]) -> f64 {
    if pixels.is_empty() {
        return 0.0;
    }
    pixels.iter().map(|&p| p as f64).sum::<f64>() / pixels.len() as f64
}

pub fn bright_pixel_count(pixels: &[u8], threshold: u8) -> u32 {
    pixels.iter().filter(|&&p| p >= threshold).count() as u32
}

pub fn write_test_pattern(
    path: &Path,
    width: u32,
    height: u32,
    inject_defect: bool,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let stride = width;
    let len = (stride * height) as usize;
    let mut buf = vec![0u8; len];
    for (i, px) in buf.iter_mut().enumerate() {
        let x = (i as u32) % stride;
        let y = (i as u32) / stride;
        *px = ((x + y) % 256) as u8;
    }
    if inject_defect {
        let cx = (stride / 2) as usize;
        let cy = (height / 2) as usize;
        for dy in 0..8 {
            for dx in 0..8 {
                let x = cx + dx;
                let y = cy + dy;
                if x < stride as usize && y < height as usize {
                    buf[y * stride as usize + x] = 250;
                }
            }
        }
    }
    std::fs::write(path, buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolves_notify_name_to_dev_shm() {
        let p = resolve_shm_path("/sfi.aoi.demo");
        assert_eq!(p, PathBuf::from("/dev/shm/sfi.aoi.demo"));
    }

    #[test]
    fn roundtrip_write_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("frame.raw");
        write_test_pattern(&path, 8, 8, true).unwrap();
        let pixels = read_gray8(path.to_str().unwrap(), 64, 0).unwrap();
        assert!(bright_pixel_count(&pixels, 128) >= 1);
    }
}

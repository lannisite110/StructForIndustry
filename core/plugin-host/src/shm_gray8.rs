//! POSIX shm Gray8 helpers for out-of-process plugins.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use memmap2::{Mmap, MmapOptions};

/// Resolve HAL notify shm name to a filesystem path under `/dev/shm`.
///
/// HAL notify uses `/sfi.aoi.line.0` (single segment). Absolute paths with
/// multiple segments (e.g. temp files in tests) are used as-is.
pub fn resolve_shm_path(shm_name: &str) -> PathBuf {
    if shm_name.starts_with("/dev/shm/") {
        PathBuf::from(shm_name)
    } else if let Some(stripped) = shm_name.strip_prefix('/') {
        if stripped.contains('/') {
            PathBuf::from(shm_name)
        } else {
            PathBuf::from(format!("/dev/shm/{stripped}"))
        }
    } else {
        PathBuf::from(format!("/dev/shm/{shm_name}"))
    }
}

pub fn read_gray8(shm_name: &str, byte_length: u64, offset: u64) -> io::Result<Vec<u8>> {
    let mmap = mmap_gray8(shm_name, byte_length, offset)?;
    Ok(mmap.to_vec())
}

/// Memory-map shm without copying the full frame into the heap.
pub fn mmap_gray8(shm_name: &str, byte_length: u64, offset: u64) -> io::Result<Mmap> {
    let path = resolve_shm_path(shm_name);
    let file = File::open(path)?;
    unsafe {
        MmapOptions::new()
            .len(byte_length as usize)
            .offset(offset)
            .map(&file)
    }
    .map_err(|e| io::Error::other(e.to_string()))
}

pub fn gray_mean(pixels: &[u8]) -> f64 {
    if pixels.is_empty() {
        return 0.0;
    }
    pixels.iter().map(|&p| p as f64).sum::<f64>() / pixels.len() as f64
}

/// Row-major gray mean over ROI; samples every 4th pixel for speed on large frames.
pub fn gray_mean_roi(
    pixels: &[u8],
    stride: u32,
    _width: u32,
    roi_x: u32,
    roi_y: u32,
    roi_w: u32,
    roi_h: u32,
) -> f64 {
    gray_mean_roi_step(pixels, stride, roi_x, roi_y, roi_w, roi_h, 4)
}

pub fn gray_mean_roi_step(
    pixels: &[u8],
    stride: u32,
    roi_x: u32,
    roi_y: u32,
    roi_w: u32,
    roi_h: u32,
    step: u32,
) -> f64 {
    let step = step.max(1);
    let mut sum = 0u64;
    let mut count = 0u64;
    for row in (0..roi_h).step_by(step as usize) {
        let y = roi_y + row;
        let row_start = (y * stride + roi_x) as usize;
        let row_end = row_start + roi_w as usize;
        if row_end > pixels.len() {
            continue;
        }
        for x in (0..roi_w).step_by(step as usize) {
            sum += pixels[row_start + x as usize] as u64;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum as f64 / count as f64
    }
}

pub fn bright_pixel_count(pixels: &[u8], threshold: u8) -> u32 {
    pixels.iter().filter(|&&p| p >= threshold).count() as u32
}

/// Count bright pixels in ROI; stops early once `limit` is reached (0 = no limit).
#[allow(clippy::too_many_arguments)]
pub fn bright_pixels_in_roi(
    pixels: &[u8],
    stride: u32,
    _width: u32,
    roi_x: u32,
    roi_y: u32,
    roi_w: u32,
    roi_h: u32,
    threshold: u8,
) -> u32 {
    bright_pixels_in_roi_limit(pixels, stride, roi_x, roi_y, roi_w, roi_h, threshold, 0)
}

#[allow(clippy::too_many_arguments)]
pub fn bright_pixels_in_roi_limit(
    pixels: &[u8],
    stride: u32,
    roi_x: u32,
    roi_y: u32,
    roi_w: u32,
    roi_h: u32,
    threshold: u8,
    limit: u32,
) -> u32 {
    let mut count = 0u32;
    for row in 0..roi_h {
        let y = roi_y + row;
        let row_start = (y * stride + roi_x) as usize;
        let row_end = row_start + roi_w as usize;
        if row_end > pixels.len() {
            continue;
        }
        for &p in &pixels[row_start..row_end] {
            if p >= threshold {
                count += 1;
                if limit > 0 && count >= limit {
                    return count;
                }
            }
        }
    }
    count
}

/// Extract ROI sub-rectangle from row-major gray8 buffer. Returns (pixels, w, h).
#[allow(clippy::too_many_arguments)]
pub fn crop_roi(
    pixels: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    roi_x: u32,
    roi_y: u32,
    roi_w: u32,
    roi_h: u32,
) -> (Vec<u8>, u32, u32) {
    let x0 = roi_x.min(width);
    let y0 = roi_y.min(height);
    let w = roi_w.min(width.saturating_sub(x0)).max(1);
    let h = roi_h.min(height.saturating_sub(y0)).max(1);
    let mut out = Vec::with_capacity((w * h) as usize);
    for row in 0..h {
        let y = y0 + row;
        let start = (y * stride + x0) as usize;
        let end = start + w as usize;
        if end <= pixels.len() {
            out.extend_from_slice(&pixels[start..end]);
        }
    }
    (out, w, h)
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

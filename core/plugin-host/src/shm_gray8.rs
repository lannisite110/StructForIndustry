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

/// Parabolic sub-pixel offset from three gradient samples (offset in [-1, 1]).
pub fn parabolic_subpixel(y0: f64, y1: f64, y2: f64) -> f64 {
    let denom = y0 - 2.0 * y1 + y2;
    if denom.abs() < 1e-6 {
        return 0.0;
    }
    0.5 * (y0 - y2) / denom
}

/// Horizontal edge caliper on row `y` between `x0` and `x1`.
/// `polarity`: `rising`, `falling`, or `both`. Returns `(subpixel_x, strength)`.
#[allow(clippy::too_many_arguments)]
pub fn edge_caliper_horizontal(
    pixels: &[u8],
    width: u32,
    height: u32,
    y: u32,
    x0: u32,
    x1: u32,
    polarity: &str,
) -> Option<(f64, f64)> {
    let width = width as i32;
    let height = height as i32;
    let mut y = y as i32;
    let mut x0 = x0 as i32;
    let mut x1 = x1 as i32;
    y = y.clamp(0, height - 1);
    x0 = x0.clamp(0, width - 1);
    x1 = x1.clamp(0, width - 1);
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
    }
    if x1 - x0 < 2 {
        return None;
    }
    let row = y * width;
    let mut best_i = 0;
    let mut best_g = -1.0;
    for x in x0..x1 {
        let g = pixels[(row + x + 2) as usize] as f64 - pixels[(row + x + 1) as usize] as f64;
        let mag = match polarity {
            "falling" => -g,
            "both" => g.abs(),
            _ => g,
        };
        if mag > best_g {
            best_g = mag;
            best_i = x;
        }
    }
    if best_g <= 0.0 {
        return None;
    }
    let i = best_i;
    let mut g0 = pixels[(row + i + 1) as usize] as f64 - pixels[(row + i) as usize] as f64;
    let g1 = pixels[(row + i + 2) as usize] as f64 - pixels[(row + i + 1) as usize] as f64;
    let mut g2 = if i + 2 < x1 {
        pixels[(row + i + 3) as usize] as f64 - pixels[(row + i + 2) as usize] as f64
    } else {
        g1
    };
    if polarity == "falling" {
        g0 = -g0;
        g2 = -g2;
    } else if polarity == "both" {
        g0 = g0.abs();
        g2 = g2.abs();
    }
    let sub = parabolic_subpixel(g0, g1, g2);
    Some((i as f64 + 0.5 + sub, best_g))
}

/// Vertical edge caliper on column `x` between `y0` and `y1`.
#[allow(clippy::too_many_arguments)]
pub fn edge_caliper_vertical(
    pixels: &[u8],
    width: u32,
    height: u32,
    x: u32,
    y0: u32,
    y1: u32,
    polarity: &str,
) -> Option<(f64, f64)> {
    let width = width as i32;
    let height = height as i32;
    let mut x = x as i32;
    let mut y0 = y0 as i32;
    let mut y1 = y1 as i32;
    x = x.clamp(0, width - 1);
    y0 = y0.clamp(0, height - 1);
    y1 = y1.clamp(0, height - 1);
    if y0 > y1 {
        std::mem::swap(&mut y0, &mut y1);
    }
    if y1 - y0 < 2 {
        return None;
    }
    let mut best_i = 0;
    let mut best_g = -1.0;
    for y in y0..y1 {
        let g = pixels[((y + 2) * width + x) as usize] as f64
            - pixels[((y + 1) * width + x) as usize] as f64;
        let mag = match polarity {
            "falling" => -g,
            "both" => g.abs(),
            _ => g,
        };
        if mag > best_g {
            best_g = mag;
            best_i = y;
        }
    }
    if best_g <= 0.0 {
        return None;
    }
    let i = best_i;
    let mut g0 = pixels[((i + 1) * width + x) as usize] as f64
        - pixels[(i * width + x) as usize] as f64;
    let g1 = pixels[((i + 2) * width + x) as usize] as f64
        - pixels[((i + 1) * width + x) as usize] as f64;
    let mut g2 = if i + 2 < y1 {
        pixels[((i + 3) * width + x) as usize] as f64
            - pixels[((i + 2) * width + x) as usize] as f64
    } else {
        g1
    };
    if polarity == "falling" {
        g0 = -g0;
        g2 = -g2;
    } else if polarity == "both" {
        g0 = g0.abs();
        g2 = g2.abs();
    }
    let sub = parabolic_subpixel(g0, g1, g2);
    Some((i as f64 + 0.5 + sub, best_g))
}

/// Horizontal line width via rising + falling edges on row `y`.
pub fn measure_line_width_horizontal(
    pixels: &[u8],
    width: u32,
    height: u32,
    y: u32,
    x0: u32,
    x1: u32,
) -> Option<(f64, f64, f64, f64)> {
    let left = edge_caliper_horizontal(pixels, width, height, y, x0, x1, "rising")?;
    let right = edge_caliper_horizontal(pixels, width, height, y, x0, x1, "falling")?;
    let w = right.0 - left.0;
    if w <= 0.0 {
        return None;
    }
    Some((w, left.0, right.0, (left.1 + right.1) / 2.0))
}

/// Disk diameter via horizontal caliper through `cy`.
pub fn measure_circle_diameter_horizontal(
    pixels: &[u8],
    width: u32,
    height: u32,
    cy: u32,
    x0: u32,
    x1: u32,
) -> Option<(f64, f64, f64, f64)> {
    let w = measure_line_width_horizontal(pixels, width, height, cy, x0, x1)?;
    Some((w.0, w.0 / 2.0, w.1, w.2))
}

fn template_stats(template: &[u8]) -> (f64, f64) {
    let n = template.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let sum = template.iter().map(|&p| p as f64).sum::<f64>();
    let mean = sum / n as f64;
    let var = template
        .iter()
        .map(|&p| {
            let d = p as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n as f64;
    (mean, var.max(0.0).sqrt())
}

/// Zero-mean NCC between `template` (tw×th) and image patch at `(x, y)`.
pub fn ncc_score_at(
    pixels: &[u8],
    width: u32,
    height: u32,
    template: &[u8],
    tw: u32,
    th: u32,
    x: u32,
    y: u32,
) -> f64 {
    if tw == 0 || th == 0 || x + tw > width || y + th > height {
        return -1.0;
    }
    let (μ_t, σ_t) = template_stats(template);
    if σ_t < 1e-6 {
        return -1.0;
    }
    let n = (tw * th) as usize;
    let mut sum_cross = 0.0;
    let mut sum_i = 0.0;
    let mut sum_i2 = 0.0;
    let width = width as usize;
    let tw = tw as usize;
    let th = th as usize;
    let x = x as usize;
    let y = y as usize;
    for row in 0..th {
        let img_row = (y + row) * width + x;
        let tpl_row = row * tw;
        for col in 0..tw {
            let iv = pixels[img_row + col] as f64;
            let tv = template[tpl_row + col] as f64;
            sum_cross += (tv - μ_t) * iv;
            sum_i += iv;
            sum_i2 += iv * iv;
        }
    }
    let μ_i = sum_i / n as f64;
    let var_i = sum_i2 / n as f64 - μ_i * μ_i;
    let σ_i = var_i.max(0.0).sqrt();
    if σ_i < 1e-6 {
        return -1.0;
    }
    sum_cross / (n as f64 * σ_t * σ_i)
}

#[allow(clippy::too_many_arguments)]
pub fn ncc_match(
    pixels: &[u8],
    width: u32,
    height: u32,
    template: &[u8],
    tw: u32,
    th: u32,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
) -> Option<(u32, u32, f64)> {
    if tw == 0 || th == 0 || width < tw || height < th {
        return None;
    }
    let max_x = width - tw;
    let max_y = height - th;
    let x0 = x0.min(max_x);
    let y0 = y0.min(max_y);
    let x1 = x1.min(max_x);
    let y1 = y1.min(max_y);
    let (x0, x1) = if x0 > x1 { (x1, x0) } else { (x0, x1) };
    let (y0, y1) = if y0 > y1 { (y1, y0) } else { (y0, y1) };
    let mut best_x = x0;
    let mut best_y = y0;
    let mut best_score = -1.0;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let score = ncc_score_at(pixels, width, height, template, tw, th, x, y);
            if score > best_score {
                best_score = score;
                best_x = x;
                best_y = y;
            }
        }
    }
    if best_score < -0.5 {
        return None;
    }
    Some((best_x, best_y, best_score))
}

pub fn extract_template(
    pixels: &[u8],
    width: u32,
    height: u32,
    tx: u32,
    ty: u32,
    tw: u32,
    th: u32,
) -> (Vec<u8>, u32, u32) {
    let tx = tx.min(width.saturating_sub(1));
    let ty = ty.min(height.saturating_sub(1));
    let tw = tw.min(width.saturating_sub(tx)).max(1);
    let th = th.min(height.saturating_sub(ty)).max(1);
    let mut out = Vec::with_capacity((tw * th) as usize);
    let width = width as usize;
    let tw = tw as usize;
    let th = th as usize;
    let tx = tx as usize;
    let ty = ty as usize;
    for row in 0..th {
        let start = (ty + row) * width + tx;
        out.extend_from_slice(&pixels[start..start + tw]);
    }
    (out, tw as u32, th as u32)
}

/// Gray8 frame with a bright template patch for NCC E2E.
pub fn write_template_pattern(
    path: &Path,
    width: u32,
    height: u32,
    tx: u32,
    ty: u32,
    tw: u32,
    th: u32,
    background: u8,
    bright: u8,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let stride = width;
    let mut buf = vec![background; (stride * height) as usize];
    let tw = tw.min(width.saturating_sub(tx));
    let th = th.min(height.saturating_sub(ty));
    for row in 0..th {
        for col in 0..tw {
            let x = tx + col;
            let y = ty + row;
            buf[y as usize * stride as usize + x as usize] = bright;
        }
    }
    std::fs::write(path, buf)
}

pub fn write_measure_edge_pattern(
    path: &Path,
    width: u32,
    height: u32,
    scan_y: u32,
    edge_x: u32,
    dark: u8,
    bright: u8,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let stride = width;
    let len = (stride * height) as usize;
    let mut buf = vec![0u8; len];
    let edge_x = edge_x.min(width.saturating_sub(1));
    let scan_y = scan_y.min(height.saturating_sub(1));
    for y in 0..height {
        for x in 0..width {
            let v = if x < edge_x { dark } else { bright };
            buf[y as usize * stride as usize + x as usize] = v;
        }
    }
    // soften edge for sub-pixel gradient (1px ramp)
    if edge_x > 0 && edge_x < width {
        let idx = scan_y as usize * stride as usize + edge_x as usize;
        buf[idx] = ((dark as u16 + bright as u16) / 2) as u8;
    }
    std::fs::write(path, buf)
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
    fn edge_caliper_finds_rising_edge() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edge.raw");
        write_measure_edge_pattern(&path, 128, 64, 32, 48, 30, 220).unwrap();
        let pixels = read_gray8(path.to_str().unwrap(), 128 * 64, 0).unwrap();
        let edge = edge_caliper_horizontal(&pixels, 128, 64, 32, 0, 127, "rising").unwrap();
        assert!(edge.0 > 45.0 && edge.0 < 52.0);
        assert!(edge.1 > 0.0);
    }

    #[test]
    fn line_width_from_edges() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bar.raw");
        let w = 200u32;
        let h = 40u32;
        let mut buf = vec![20u8; (w * h) as usize];
        for y in 0..h {
            for x in 40..80 {
                buf[y as usize * w as usize + x] = 200;
            }
        }
        std::fs::write(&path, buf).unwrap();
        let pixels = read_gray8(path.to_str().unwrap(), (w * h) as u64, 0).unwrap();
        let m = measure_line_width_horizontal(&pixels, w, h, 20, 0, 199).unwrap();
        assert!(m.0 > 35.0 && m.0 < 45.0);
    }

    #[test]
    fn ncc_finds_template_patch() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tpl.raw");
        write_template_pattern(&path, 64, 48, 20, 12, 16, 16, 40, 200).unwrap();
        let mut pixels = read_gray8(path.to_str().unwrap(), 64 * 48, 0).unwrap();
        pixels[12 * 64 + 20] = 180;
        let (tpl, tw, th) = extract_template(&pixels, 64, 48, 20, 12, 16, 16);
        let m = ncc_match(&pixels, 64, 48, &tpl, tw, th, 0, 0, 63, 47).unwrap();
        assert_eq!(m.0, 20);
        assert_eq!(m.1, 12);
        assert!(m.2 > 0.99);
    }
}

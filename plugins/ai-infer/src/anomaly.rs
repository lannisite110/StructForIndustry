//! OK-only anomaly detection (PatchCore/EfficientAD-lite, pure Rust).
//!
//! Calibrate from defect-free (OK) frames only: split each frame into a fixed
//! grid of cells, describe every cell with a small texture descriptor, and keep
//! a per-cell memory bank of OK descriptors. At inference time the anomaly score
//! of a cell is its nearest-neighbour distance to the OK bank; the image score
//! is the worst cell. A leave-one-out pass over the OK set sets the threshold.
//!
//! This is a faithful, dependency-free stand-in for a learned feature extractor:
//! the same calibrate → bank → NN-distance → threshold pipeline as PatchCore,
//! with a hand-crafted descriptor instead of a CNN. Swap in ONNX features later
//! without changing the bus contract.

use serde::{Deserialize, Serialize};

/// Descriptor dimensions per cell: [mean, std, mean_abs_gradient].
pub const DESCRIPTOR_DIM: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyModel {
    pub grid_w: u32,
    pub grid_h: u32,
    /// Subtract per-frame global mean from cell mean (illumination robustness).
    pub normalize_illumination: bool,
    /// Per-cell OK descriptors: `bank[cell][sample][DESCRIPTOR_DIM]`.
    pub bank: Vec<Vec<[f32; DESCRIPTOR_DIM]>>,
    /// Image-level anomaly threshold (score above => NG).
    pub threshold: f32,
    /// Diagnostics from calibration.
    pub ok_score_max: f32,
    pub ok_score_mean: f32,
    pub ok_sample_count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct AnomalyResult {
    pub score: f32,
    pub threshold: f32,
    /// Worst cell column/row (grid coordinates).
    pub worst_col: u32,
    pub worst_row: u32,
}

impl AnomalyResult {
    pub fn is_defect(&self) -> bool {
        self.score > self.threshold
    }
}

fn cell_bounds(idx: u32, count: u32, total: u32) -> (usize, usize) {
    let start = (idx as u64 * total as u64 / count as u64) as usize;
    let end = ((idx + 1) as u64 * total as u64 / count as u64) as usize;
    (start, end.max(start + 1))
}

/// Describe a single cell: mean, std, mean abs gradient.
///
/// When `normalize_illumination` is set the descriptor is standardized by the
/// frame's global mean/std, making it invariant to affine lighting changes
/// (`gain * pixel + offset`) — both additive offset and multiplicative gain.
#[allow(clippy::too_many_arguments)]
fn describe_cell(
    pixels: &[u8],
    width: usize,
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    global_mean: f32,
    global_std: f32,
    normalize_illumination: bool,
) -> [f32; DESCRIPTOR_DIM] {
    if x0 >= x1 || y0 >= y1 {
        return [0.0; DESCRIPTOR_DIM];
    }
    let mut sum = 0.0f32;
    let mut sum_sq = 0.0f32;
    let mut grad = 0.0f32;
    let mut n = 0.0f32;
    for y in y0..y1 {
        let row = y * width;
        for x in x0..x1 {
            let v = pixels[row + x] as f32 / 255.0;
            sum += v;
            sum_sq += v * v;
            let right = if x + 1 < x1 {
                pixels[row + x + 1] as f32 / 255.0
            } else {
                v
            };
            let down = if y + 1 < y1 {
                pixels[(y + 1) * width + x] as f32 / 255.0
            } else {
                v
            };
            grad += (right - v).abs() + (down - v).abs();
            n += 1.0;
        }
    }
    let mean = sum / n;
    let var = (sum_sq / n - mean * mean).max(0.0);
    let std = var.sqrt();
    let grad_mean = grad / n;
    if normalize_illumination {
        let inv = 1.0 / (global_std + 1e-3);
        [(mean - global_mean) * inv, std * inv, grad_mean * inv]
    } else {
        [mean, std, grad_mean]
    }
}

/// Frame-global mean and std of normalized pixels.
fn frame_stats(pixels: &[u8], len: usize) -> (f32, f32) {
    if len == 0 {
        return (0.0, 0.0);
    }
    let mut sum = 0.0f32;
    let mut sum_sq = 0.0f32;
    for &p in &pixels[..len] {
        let v = p as f32 / 255.0;
        sum += v;
        sum_sq += v * v;
    }
    let mean = sum / len as f32;
    let std = (sum_sq / len as f32 - mean * mean).max(0.0).sqrt();
    (mean, std)
}

/// Descriptor for every cell of a frame, row-major by cell.
fn describe_frame(
    model_grid_w: u32,
    model_grid_h: u32,
    normalize_illumination: bool,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Option<Vec<[f32; DESCRIPTOR_DIM]>> {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || pixels.len() < w * h {
        return None;
    }
    let (gm, gs) = if normalize_illumination {
        frame_stats(pixels, w * h)
    } else {
        (0.0, 0.0)
    };
    let mut out = Vec::with_capacity((model_grid_w * model_grid_h) as usize);
    for gy in 0..model_grid_h {
        let (y0, y1) = cell_bounds(gy, model_grid_h, height);
        let y1 = y1.min(h);
        for gx in 0..model_grid_w {
            let (x0, x1) = cell_bounds(gx, model_grid_w, width);
            let x1 = x1.min(w);
            out.push(describe_cell(
                pixels,
                w,
                x0,
                x1,
                y0,
                y1,
                gm,
                gs,
                normalize_illumination,
            ));
        }
    }
    Some(out)
}

fn dist_sq(a: &[f32; DESCRIPTOR_DIM], b: &[f32; DESCRIPTOR_DIM]) -> f32 {
    let mut d = 0.0;
    for i in 0..DESCRIPTOR_DIM {
        let diff = a[i] - b[i];
        d += diff * diff;
    }
    d
}

/// Nearest-neighbour distance of `desc` to a cell's OK bank, optionally skipping
/// one bank index (leave-one-out during calibration).
fn nn_distance(
    bank: &[[f32; DESCRIPTOR_DIM]],
    desc: &[f32; DESCRIPTOR_DIM],
    skip: Option<usize>,
) -> f32 {
    let mut best = f32::INFINITY;
    for (i, b) in bank.iter().enumerate() {
        if Some(i) == skip {
            continue;
        }
        let d = dist_sq(b, desc);
        if d < best {
            best = d;
        }
    }
    if best.is_finite() {
        best.sqrt()
    } else {
        0.0
    }
}

/// Configuration for calibration.
#[derive(Debug, Clone, Copy)]
pub struct CalibrateConfig {
    pub grid_w: u32,
    pub grid_h: u32,
    pub normalize_illumination: bool,
    /// threshold = max(leave-one-out OK score) * margin.
    pub threshold_margin: f32,
}

impl Default for CalibrateConfig {
    fn default() -> Self {
        Self {
            grid_w: 16,
            grid_h: 16,
            normalize_illumination: true,
            threshold_margin: 1.3,
        }
    }
}

/// Calibrate an OK-only model. Each frame is `(pixels, width, height)`.
pub fn calibrate(
    frames: &[(Vec<u8>, u32, u32)],
    cfg: &CalibrateConfig,
) -> Result<AnomalyModel, String> {
    if frames.is_empty() {
        return Err("no OK frames for calibration".into());
    }
    let cells = (cfg.grid_w * cfg.grid_h) as usize;
    let mut bank: Vec<Vec<[f32; DESCRIPTOR_DIM]>> = vec![Vec::with_capacity(frames.len()); cells];

    for (pixels, w, h) in frames {
        let desc = describe_frame(
            cfg.grid_w,
            cfg.grid_h,
            cfg.normalize_illumination,
            pixels,
            *w,
            *h,
        )
        .ok_or_else(|| format!("bad OK frame {w}x{h}"))?;
        for (cell, d) in desc.into_iter().enumerate() {
            bank[cell].push(d);
        }
    }

    // Leave-one-out OK scores to set the threshold.
    let mut ok_scores = Vec::with_capacity(frames.len());
    for sample in 0..frames.len() {
        let mut worst = 0.0f32;
        for cell in bank.iter() {
            if sample >= cell.len() {
                continue;
            }
            let d = nn_distance(cell, &cell[sample], Some(sample));
            if d > worst {
                worst = d;
            }
        }
        ok_scores.push(worst);
    }

    let ok_score_max = ok_scores.iter().cloned().fold(0.0f32, f32::max);
    let ok_score_mean = ok_scores.iter().sum::<f32>() / ok_scores.len() as f32;
    // With a single OK frame leave-one-out is degenerate; fall back to a small floor.
    let threshold = if ok_score_max > 0.0 {
        ok_score_max * cfg.threshold_margin
    } else {
        0.05
    };

    Ok(AnomalyModel {
        grid_w: cfg.grid_w,
        grid_h: cfg.grid_h,
        normalize_illumination: cfg.normalize_illumination,
        bank,
        threshold,
        ok_score_max,
        ok_score_mean,
        ok_sample_count: frames.len() as u32,
    })
}

impl AnomalyModel {
    /// Score a frame; image score = worst cell NN distance.
    pub fn score(&self, pixels: &[u8], width: u32, height: u32) -> Option<AnomalyResult> {
        let desc = describe_frame(
            self.grid_w,
            self.grid_h,
            self.normalize_illumination,
            pixels,
            width,
            height,
        )?;
        let mut worst = 0.0f32;
        let mut worst_cell = 0usize;
        for (cell, d) in desc.iter().enumerate() {
            if self.bank[cell].is_empty() {
                continue;
            }
            let dist = nn_distance(&self.bank[cell], d, None);
            if dist > worst {
                worst = dist;
                worst_cell = cell;
            }
        }
        Some(AnomalyResult {
            score: worst,
            threshold: self.threshold,
            worst_col: worst_cell as u32 % self.grid_w,
            worst_row: worst_cell as u32 / self.grid_w,
        })
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_json(&text)
    }
}

pub fn model_path_from_env() -> Option<std::path::PathBuf> {
    std::env::var("SFI_ANOMALY_MODEL")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::synthetic::{defect_surface, ok_surface, HEIGHT as H, WIDTH as W};

    fn ok_frame(seed: u64) -> Vec<u8> {
        ok_surface(seed)
    }

    fn defect_frame() -> Vec<u8> {
        defect_surface(0)
    }

    fn ok_set(n: u32) -> Vec<(Vec<u8>, u32, u32)> {
        (0..n).map(|s| (ok_surface(s as u64), W, H)).collect()
    }

    #[test]
    fn calibrate_and_detect_defect() {
        let model = calibrate(&ok_set(10), &CalibrateConfig::default()).unwrap();
        let ok = model.score(&ok_frame(100), W, H).unwrap();
        let ng = model.score(&defect_frame(), W, H).unwrap();
        assert!(!ok.is_defect(), "OK frame flagged: {ok:?}");
        assert!(ng.is_defect(), "defect missed: {ng:?}");
        assert!(ng.score > ok.score);
    }

    #[test]
    fn worst_cell_localizes_defect() {
        let model = calibrate(&ok_set(8), &CalibrateConfig::default()).unwrap();
        let ng = model.score(&defect_frame(), W, H).unwrap();
        // Defect is centered, so worst cell should be near the middle of the grid.
        assert!(ng.worst_col >= model.grid_w / 4 && ng.worst_col <= 3 * model.grid_w / 4);
        assert!(ng.worst_row >= model.grid_h / 4 && ng.worst_row <= 3 * model.grid_h / 4);
    }

    #[test]
    fn json_roundtrip() {
        let model = calibrate(&ok_set(5), &CalibrateConfig::default()).unwrap();
        let json = model.to_json().unwrap();
        let back = AnomalyModel::from_json(&json).unwrap();
        let a = model.score(&defect_frame(), W, H).unwrap();
        let b = back.score(&defect_frame(), W, H).unwrap();
        assert_eq!(a.score, b.score);
    }
}

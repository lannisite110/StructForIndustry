//! OK-only anomaly detection (PatchCore/EfficientAD-lite).
//!
//! Calibrate from defect-free (OK) frames only: split each frame into a fixed
//! grid of cells, describe every cell with a feature vector, and keep a per-cell
//! memory bank of OK descriptors. At inference time the anomaly score of a cell
//! is its nearest-neighbour distance to the OK bank; the image score is the
//! worst cell. A leave-one-out pass over the OK set sets the threshold.
//!
//! The descriptor (feature extractor) is pluggable, the calibrate → bank →
//! NN-distance → threshold pipeline is identical regardless:
//!   - [`Extractor::Handcrafted`] — `[mean, std, mean_abs_gradient]` per cell.
//!   - [`Extractor::Onnx`] — real CNN features from an ONNX model via `ort`
//!     (`--features onnx`); when the runtime/model is unavailable it falls back
//!     to a deterministic filter-bank emulation so the pipeline stays usable in
//!     CI without `libonnxruntime`.

use serde::{Deserialize, Serialize};

/// Handcrafted descriptor width: `[mean, std, mean_abs_gradient]`.
pub const HANDCRAFTED_DIM: usize = 3;
/// Filter-bank (ONNX reference) descriptor width.
pub const FILTERBANK_DIM: usize = 5;

/// Per-cell feature extractor. Stored in the model so scoring matches calibration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Extractor {
    /// `[mean, std, mean_abs_gradient]` (illumination-normalized).
    #[default]
    Handcrafted,
    /// CNN features from an ONNX model; falls back to a filter-bank emulation
    /// when `ort`/the model are unavailable. `model` may be empty for the
    /// reference emulation only.
    Onnx { model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyModel {
    pub grid_w: u32,
    pub grid_h: u32,
    /// Standardize each frame by its global mean/std (affine-light invariance).
    pub normalize_illumination: bool,
    /// Feature extractor used to build `bank` (and required at scoring time).
    #[serde(default)]
    pub extractor: Extractor,
    /// Descriptor dimensionality (per cell).
    pub descriptor_dim: u32,
    /// Per-cell OK descriptors: `bank[cell][sample]` is a `descriptor_dim` vec.
    pub bank: Vec<Vec<Vec<f32>>>,
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

/// Standardized pixel value at `(x, y)` (affine-light invariant when enabled).
#[inline]
fn norm_at(
    pixels: &[u8],
    width: usize,
    x: usize,
    y: usize,
    gm: f32,
    inv_gs: f32,
    normalize: bool,
) -> f32 {
    let v = pixels[y * width + x] as f32 / 255.0;
    if normalize {
        (v - gm) * inv_gs
    } else {
        v
    }
}

/// Handcrafted `[mean, std, mean_abs_gradient]` for one cell.
#[allow(clippy::too_many_arguments)]
fn cell_handcrafted(
    pixels: &[u8],
    width: usize,
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    gm: f32,
    inv_gs: f32,
    normalize: bool,
) -> Vec<f32> {
    let mut sum = 0.0f32;
    let mut sum_sq = 0.0f32;
    let mut grad = 0.0f32;
    let mut n = 0.0f32;
    for y in y0..y1 {
        for x in x0..x1 {
            let v = norm_at(pixels, width, x, y, gm, inv_gs, normalize);
            sum += v;
            sum_sq += v * v;
            let right = if x + 1 < x1 {
                norm_at(pixels, width, x + 1, y, gm, inv_gs, normalize)
            } else {
                v
            };
            let down = if y + 1 < y1 {
                norm_at(pixels, width, x, y + 1, gm, inv_gs, normalize)
            } else {
                v
            };
            grad += (right - v).abs() + (down - v).abs();
            n += 1.0;
        }
    }
    let mean = sum / n;
    let var = (sum_sq / n - mean * mean).max(0.0);
    vec![mean, var.sqrt(), grad / n]
}

/// Filter-bank descriptor for one cell — a deterministic stand-in for early CNN
/// feature maps: `[mean, std, |∂x|, |∂y|, |Laplacian|]` over standardized pixels.
#[allow(clippy::too_many_arguments)]
fn cell_filterbank(
    pixels: &[u8],
    width: usize,
    height: usize,
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    gm: f32,
    inv_gs: f32,
    normalize: bool,
) -> Vec<f32> {
    let at = |x: usize, y: usize| norm_at(pixels, width, x, y, gm, inv_gs, normalize);
    let mut mean = 0.0f32;
    let mut sum_sq = 0.0f32;
    let mut gx = 0.0f32;
    let mut gy = 0.0f32;
    let mut lap = 0.0f32;
    let mut n = 0.0f32;
    for y in y0..y1 {
        for x in x0..x1 {
            let c = at(x, y);
            let l = if x > 0 { at(x - 1, y) } else { c };
            let r = if x + 1 < width { at(x + 1, y) } else { c };
            let u = if y > 0 { at(x, y - 1) } else { c };
            let d = if y + 1 < height { at(x, y + 1) } else { c };
            mean += c;
            sum_sq += c * c;
            gx += (r - l).abs();
            gy += (d - u).abs();
            lap += (l + r + u + d - 4.0 * c).abs();
            n += 1.0;
        }
    }
    let mean = mean / n;
    let var = (sum_sq / n - mean * mean).max(0.0);
    vec![mean, var.sqrt(), gx / n, gy / n, lap / n]
}

#[cfg(feature = "onnx")]
mod onnx_features {
    use super::*;
    use ort::{GraphOptimizationLevel, Session};
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Mutex;

    static SESSIONS: Mutex<Option<HashMap<String, Option<&'static Session>>>> = Mutex::new(None);

    fn session(model: &str) -> Option<&'static Session> {
        let mut guard = SESSIONS.lock().ok()?;
        let map = guard.get_or_insert_with(HashMap::new);
        if let Some(slot) = map.get(model) {
            return *slot;
        }
        let built = Session::builder()
            .ok()
            .and_then(|b| {
                b.with_optimization_level(GraphOptimizationLevel::Level1)
                    .ok()
            })
            .and_then(|b| b.commit_from_file(Path::new(model)).ok())
            .map(|s| &*Box::leak(Box::new(s)));
        map.insert(model.to_string(), built);
        built
    }

    /// Run an ONNX feature extractor producing `[1, C, Hf, Wf]`, pool the feature
    /// map onto the requested grid, returning one `C`-dim descriptor per cell.
    #[allow(clippy::too_many_arguments)]
    pub fn extract(
        model: &str,
        pixels: &[u8],
        width: u32,
        height: u32,
        grid_w: u32,
        grid_h: u32,
        gm: f32,
        inv_gs: f32,
        normalize: bool,
    ) -> Option<(Vec<Vec<f32>>, usize)> {
        let session = session(model)?;
        let w = width as usize;
        let h = height as usize;
        let mut input = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                input.push(norm_at(pixels, w, x, y, gm, inv_gs, normalize));
            }
        }
        let tensor = ort::value::Tensor::from_array(([1usize, 1, h, w], input)).ok()?;
        let outputs = session.run(ort::inputs![tensor]).ok()?;
        let (shape, data) = outputs[0].try_extract_tensor::<f32>().ok()?;
        // Expect [1, C, Hf, Wf].
        if shape.len() != 4 {
            return None;
        }
        let c = shape[1] as usize;
        let hf = shape[2] as usize;
        let wf = shape[3] as usize;
        if c == 0 || hf == 0 || wf == 0 {
            return None;
        }
        let mut cells = Vec::with_capacity((grid_w * grid_h) as usize);
        for gy in 0..grid_h {
            let fy = (gy as usize * hf / grid_h as usize).min(hf - 1);
            for gx in 0..grid_w {
                let fx = (gx as usize * wf / grid_w as usize).min(wf - 1);
                let mut desc = Vec::with_capacity(c);
                for ch in 0..c {
                    let idx = ((ch * hf) + fy) * wf + fx;
                    desc.push(data.get(idx).copied().unwrap_or(0.0));
                }
                cells.push(desc);
            }
        }
        Some((cells, c))
    }
}

/// Descriptors for every cell of a frame (row-major), plus descriptor dim.
fn describe_frame(
    extractor: &Extractor,
    grid_w: u32,
    grid_h: u32,
    normalize: bool,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Option<(Vec<Vec<f32>>, usize)> {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || pixels.len() < w * h {
        return None;
    }
    let (gm, gs) = if normalize {
        frame_stats(pixels, w * h)
    } else {
        (0.0, 1.0)
    };
    let inv_gs = if normalize { 1.0 / (gs + 1e-3) } else { 1.0 };

    // Real ONNX features (or the filter-bank fallback) for the Onnx extractor.
    if let Extractor::Onnx { model } = extractor {
        #[cfg(feature = "onnx")]
        if !model.is_empty() {
            if let Some(out) = onnx_features::extract(
                model, pixels, width, height, grid_w, grid_h, gm, inv_gs, normalize,
            ) {
                return Some(out);
            }
        }
        let _ = model;
        let mut cells = Vec::with_capacity((grid_w * grid_h) as usize);
        for gy in 0..grid_h {
            let (y0, y1) = cell_bounds(gy, grid_h, height);
            let y1 = y1.min(h);
            for gx in 0..grid_w {
                let (x0, x1) = cell_bounds(gx, grid_w, width);
                let x1 = x1.min(w);
                cells.push(cell_filterbank(
                    pixels, w, h, x0, x1, y0, y1, gm, inv_gs, normalize,
                ));
            }
        }
        return Some((cells, FILTERBANK_DIM));
    }

    let mut cells = Vec::with_capacity((grid_w * grid_h) as usize);
    for gy in 0..grid_h {
        let (y0, y1) = cell_bounds(gy, grid_h, height);
        let y1 = y1.min(h);
        for gx in 0..grid_w {
            let (x0, x1) = cell_bounds(gx, grid_w, width);
            let x1 = x1.min(w);
            cells.push(cell_handcrafted(
                pixels, w, x0, x1, y0, y1, gm, inv_gs, normalize,
            ));
        }
    }
    Some((cells, HANDCRAFTED_DIM))
}

fn dist_sq(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

/// Nearest-neighbour distance of `desc` to a cell's OK bank, optionally skipping
/// one bank index (leave-one-out during calibration).
fn nn_distance(bank: &[Vec<f32>], desc: &[f32], skip: Option<usize>) -> f32 {
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
#[derive(Debug, Clone)]
pub struct CalibrateConfig {
    pub grid_w: u32,
    pub grid_h: u32,
    pub normalize_illumination: bool,
    /// threshold = max(leave-one-out OK score) * margin.
    pub threshold_margin: f32,
    pub extractor: Extractor,
}

impl Default for CalibrateConfig {
    fn default() -> Self {
        Self {
            grid_w: 16,
            grid_h: 16,
            normalize_illumination: true,
            threshold_margin: 1.3,
            extractor: Extractor::default(),
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
    let mut bank: Vec<Vec<Vec<f32>>> = vec![Vec::with_capacity(frames.len()); cells];
    let mut descriptor_dim = 0usize;

    for (pixels, w, h) in frames {
        let (desc, dim) = describe_frame(
            &cfg.extractor,
            cfg.grid_w,
            cfg.grid_h,
            cfg.normalize_illumination,
            pixels,
            *w,
            *h,
        )
        .ok_or_else(|| format!("bad OK frame {w}x{h}"))?;
        descriptor_dim = dim;
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
    let threshold = if ok_score_max > 0.0 {
        ok_score_max * cfg.threshold_margin
    } else {
        0.05
    };

    Ok(AnomalyModel {
        grid_w: cfg.grid_w,
        grid_h: cfg.grid_h,
        normalize_illumination: cfg.normalize_illumination,
        extractor: cfg.extractor.clone(),
        descriptor_dim: descriptor_dim as u32,
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
        let (desc, _dim) = describe_frame(
            &self.extractor,
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

    fn ok_set(n: u32) -> Vec<(Vec<u8>, u32, u32)> {
        (0..n).map(|s| (ok_surface(s as u64), W, H)).collect()
    }

    fn cfg_with(extractor: Extractor) -> CalibrateConfig {
        CalibrateConfig {
            extractor,
            ..Default::default()
        }
    }

    #[test]
    fn handcrafted_detects_defect() {
        let model = calibrate(&ok_set(10), &cfg_with(Extractor::Handcrafted)).unwrap();
        assert_eq!(model.descriptor_dim, HANDCRAFTED_DIM as u32);
        let ok = model.score(&ok_surface(100), W, H).unwrap();
        let ng = model.score(&defect_surface(0), W, H).unwrap();
        assert!(!ok.is_defect(), "OK flagged: {ok:?}");
        assert!(ng.is_defect(), "defect missed: {ng:?}");
    }

    #[test]
    fn onnx_reference_detects_defect() {
        let model = calibrate(
            &ok_set(10),
            &cfg_with(Extractor::Onnx {
                model: String::new(),
            }),
        )
        .unwrap();
        assert_eq!(model.descriptor_dim, FILTERBANK_DIM as u32);
        let ok = model.score(&ok_surface(100), W, H).unwrap();
        let ng = model.score(&defect_surface(0), W, H).unwrap();
        assert!(!ok.is_defect(), "OK flagged: {ok:?}");
        assert!(ng.is_defect(), "defect missed: {ng:?}");
        assert!(ng.score > ok.score);
    }

    #[test]
    fn worst_cell_localizes_defect() {
        let model = calibrate(
            &ok_set(8),
            &cfg_with(Extractor::Onnx {
                model: String::new(),
            }),
        )
        .unwrap();
        let ng = model.score(&defect_surface(0), W, H).unwrap();
        assert!(ng.worst_col >= model.grid_w / 4 && ng.worst_col <= 3 * model.grid_w / 4);
        assert!(ng.worst_row >= model.grid_h / 4 && ng.worst_row <= 3 * model.grid_h / 4);
    }

    #[test]
    fn json_roundtrip_preserves_extractor() {
        let model = calibrate(
            &ok_set(5),
            &cfg_with(Extractor::Onnx {
                model: String::new(),
            }),
        )
        .unwrap();
        let back = AnomalyModel::from_json(&model.to_json().unwrap()).unwrap();
        assert!(matches!(back.extractor, Extractor::Onnx { .. }));
        let a = model.score(&defect_surface(0), W, H).unwrap();
        let b = back.score(&defect_surface(0), W, H).unwrap();
        assert_eq!(a.score, b.score);
    }
}

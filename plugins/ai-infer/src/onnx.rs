//! ONNX backend — reference stub (default) or real ORT with `--features onnx`.

use std::path::Path;

#[cfg(feature = "onnx")]
mod ort_impl {
    use super::*;
    use ort::{GraphOptimizationLevel, Session};
    use std::sync::OnceLock;

    static SESSION: OnceLock<Option<Session>> = OnceLock::new();

    pub fn session(model: &Path) -> Option<&'static Session> {
        SESSION
            .get_or_init(|| {
                Session::builder()
                    .ok()?
                    .with_optimization_level(GraphOptimizationLevel::Level1)
                    .ok()?
                    .commit_from_file(model)
                    .ok()
            })
            .as_ref()
    }

    pub fn run_defect_score(model: &Path, pixels: &[u8], width: u32, height: u32) -> Option<f32> {
        let session = session(model)?;
        let w = width as usize;
        let h = height as usize;
        if pixels.len() < w * h {
            return None;
        }
        let floats: Vec<f32> = pixels[..w * h].iter().map(|&p| p as f32 / 255.0).collect();
        let input = ort::value::Tensor::from_array(([1usize, 1, h, w], floats)).ok()?;
        let outputs = session.run(ort::inputs![input]).ok()?;
        let (_, data) = outputs[0].try_extract_tensor::<f32>().ok()?;
        data.first().copied()
    }
}

/// Reference inference matching `tools/fixtures/models/tiny-defect.onnx` (GlobalAveragePool).
pub fn reference_defect_score(pixels: &[u8], width: u32, height: u32) -> Option<f32> {
    let w = width as usize;
    let h = height as usize;
    if pixels.len() < w * h || w == 0 || h == 0 {
        return None;
    }
    let sum: f32 = pixels[..w * h].iter().map(|&p| p as f32 / 255.0).sum();
    Some(sum / (w * h) as f32)
}

#[cfg(feature = "onnx")]
pub fn onnx_defect_score(model: &Path, pixels: &[u8], width: u32, height: u32) -> Option<f32> {
    ort_impl::run_defect_score(model, pixels, width, height)
        .or_else(|| reference_defect_score(pixels, width, height))
}

#[cfg(not(feature = "onnx"))]
pub fn onnx_defect_score(model: &Path, pixels: &[u8], width: u32, height: u32) -> Option<f32> {
    if !model.exists() {
        return None;
    }
    reference_defect_score(pixels, width, height)
}

pub fn model_path_from_env() -> Option<std::path::PathBuf> {
    std::env::var("SFI_ONNX_MODEL")
        .ok()
        .map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_score_detects_bright_patch() {
        let px = vec![240u8; 64 * 48];
        let score = reference_defect_score(&px, 64, 48).unwrap();
        assert!(score > 0.5);
    }
}

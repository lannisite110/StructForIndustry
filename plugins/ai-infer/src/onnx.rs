//! Optional ONNX backend — enabled with `--features onnx` and `SFI_ONNX_MODEL`.

use std::path::Path;
use std::sync::OnceLock;

#[cfg(feature = "onnx")]
mod ort_impl {
    use super::*;
    use ort::{GraphOptimizationLevel, Session};

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

#[cfg(feature = "onnx")]
pub fn onnx_defect_score(model: &Path, pixels: &[u8], width: u32, height: u32) -> Option<f32> {
    ort_impl::run_defect_score(model, pixels, width, height)
}

#[cfg(not(feature = "onnx"))]
pub fn onnx_defect_score(_model: &Path, _pixels: &[u8], _width: u32, _height: u32) -> Option<f32> {
    None
}

pub fn model_path_from_env() -> Option<std::path::PathBuf> {
    std::env::var("SFI_ONNX_MODEL").ok().map(std::path::PathBuf::from)
}

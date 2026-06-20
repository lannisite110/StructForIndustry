//! Load Gray8 frames from bench-rig directories (`tools/fixtures/bench/`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::synthetic::{HEIGHT, STRIDE, WIDTH};

#[derive(Debug, Clone)]
pub struct Gray8Frame {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub enum BenchError {
    Io(String),
    Invalid(String),
}

impl std::fmt::Display for BenchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchError::Io(e) => write!(f, "io: {e}"),
            BenchError::Invalid(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BenchError {}

pub fn default_bench_root() -> PathBuf {
    PathBuf::from("tools/fixtures/bench")
}

/// Load all `*.gray8` / `*.raw` files from `dir` (sorted by name).
pub fn load_gray8_dir(dir: &Path, width: u32, height: u32) -> Result<Vec<Gray8Frame>, BenchError> {
    if !dir.is_dir() {
        return Err(BenchError::Invalid(format!("not a directory: {}", dir.display())));
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| BenchError::Io(e.to_string()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|ext| {
                ext == "gray8" || ext == "raw" || ext == "bin"
            })
        })
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(BenchError::Invalid(format!("no gray8 frames in {}", dir.display())));
    }
    let expected = (width * height) as usize;
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::read(&path).map_err(|e| BenchError::Io(e.to_string()))?;
        if bytes.len() != expected {
            return Err(BenchError::Invalid(format!(
                "{}: expected {} bytes ({}x{}), got {}",
                path.display(),
                expected,
                width,
                height,
                bytes.len()
            )));
        }
        out.push(Gray8Frame {
            pixels: bytes,
            width,
            height,
        });
    }
    Ok(out)
}

pub fn load_ok_frames(root: &Path, width: u32, height: u32) -> Result<Vec<(Vec<u8>, u32, u32)>, BenchError> {
    let dir = root.join("ok");
    Ok(load_gray8_dir(&dir, width, height)?
        .into_iter()
        .map(|f| (f.pixels, f.width, f.height))
        .collect())
}

pub fn load_defect_frames(
    root: &Path,
    width: u32,
    height: u32,
) -> Result<Vec<(Vec<u8>, u32, u32)>, BenchError> {
    let dir = root.join("defect");
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    Ok(load_gray8_dir(&dir, width, height)?
        .into_iter()
        .map(|f| (f.pixels, f.width, f.height))
        .collect())
}

pub fn bench_root_exists(root: &Path) -> bool {
    root.join("ok").is_dir() && !fs::read_dir(root.join("ok"))
        .map(|rd| rd.filter_map(|e| e.ok()).count())
        .unwrap_or(0)
        > 0
}

/// Write synthetic OK/defect frames into `tools/fixtures/bench/` for CI and offline reports.
pub fn write_synthetic_bench_tree(root: &Path) -> Result<(), BenchError> {
    use crate::synthetic::{defect_at, defect_surface, ok_surface, ok_surface_amp};

    let ok_dir = root.join("ok");
    let defect_dir = root.join("defect");
    fs::create_dir_all(&ok_dir).map_err(|e| BenchError::Io(e.to_string()))?;
    fs::create_dir_all(&defect_dir).map_err(|e| BenchError::Io(e.to_string()))?;

    for i in 0..20u64 {
        let path = ok_dir.join(format!("{:02}.gray8", i));
        let frame = ok_surface(i);
        fs::write(&path, &frame).map_err(|e| BenchError::Io(e.to_string()))?;
    }
    for (i, seed) in (0..10u64).enumerate() {
        let path = ok_dir.join(format!("noisy_{:02}.gray8", i));
        let amp = 5 + (i as i32 % 6);
        let frame = ok_surface_amp(seed + 2040, amp);
        fs::write(&path, &frame).map_err(|e| BenchError::Io(e.to_string()))?;
    }

    fs::write(defect_dir.join("center.gray8"), defect_surface(0))
        .map_err(|e| BenchError::Io(e.to_string()))?;
    for (i, (cx, cy)) in [(12, 8), (28, 16), (44, 28)].iter().enumerate() {
        let frame = defect_at(3000 + i as u64, *cx, *cy, 6, 200);
        let path = defect_dir.join(format!("defect_{:02}.gray8", i));
        fs::write(&path, &frame).map_err(|e| BenchError::Io(e.to_string()))?;
    }

    let readme = format!(
        "# Bench Gray8 fixtures ({WIDTH}x{HEIGHT})\n\n\
         Populated by `tools/scripts/bench-fixtures-generate.sh`.\n\
         Replace files under `ok/` and `defect/` with experiment-bench captures (same byte length).\n"
    );
    fs::write(root.join("README.md"), readme).map_err(|e| BenchError::Io(e.to_string()))?;
    Ok(())
}

pub fn default_frame_dims() -> (u32, u32) {
    (WIDTH, HEIGHT)
}

pub fn default_stride() -> u32 {
    STRIDE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_and_loads_bench_tree() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("sfi-bench-test-{stamp}"));
        write_synthetic_bench_tree(&dir).unwrap();
        let ok = load_ok_frames(&dir, WIDTH, HEIGHT).unwrap();
        assert!(ok.len() >= 20);
        let defect = load_defect_frames(&dir, WIDTH, HEIGHT).unwrap();
        assert!(!defect.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

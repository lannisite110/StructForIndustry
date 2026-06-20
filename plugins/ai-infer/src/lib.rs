//! ai-infer shared library: anomaly detection + ONNX scoring.

pub mod anomaly;
pub mod onnx;

/// Synthetic Gray8 frames for OK-only calibration, E2E, and the three reports.
///
/// `ok_surface` models a smooth mid-gray inspected surface (e.g. machined
/// metal): low-contrast, unsaturated, with deterministic sensor noise. This is
/// a far better OK proxy than a full-range ramp and keeps affine-illumination
/// transforms within [0,255] so the ablation reflects the model, not clipping.
pub mod synthetic {
    pub const WIDTH: u32 = 64;
    pub const HEIGHT: u32 = 48;
    pub const STRIDE: u32 = 64;

    /// Sensor-noise amplitude (DN) for synthesized OK frames.
    pub const DITHER_AMP: i32 = 3;

    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state
    }

    /// Smooth mid-gray surface (~108..128 DN) + deterministic dither.
    pub fn ok_surface(seed: u64) -> Vec<u8> {
        let mut buf = vec![0u8; (STRIDE * HEIGHT) as usize];
        let mut state = seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1);
        for (i, px) in buf.iter_mut().enumerate() {
            let x = (i as u32) % STRIDE;
            let y = (i as u32) / STRIDE;
            // Low-frequency gradient field, no wrap, no saturation.
            let base = 108.0 + 10.0 * (x as f32 / WIDTH as f32) + 8.0 * (y as f32 / HEIGHT as f32);
            let r = (lcg(&mut state) >> 40) as i64 % (2 * DITHER_AMP as i64 + 1);
            let dither = r as i32 - DITHER_AMP;
            *px = (base as i32 + dither).clamp(0, 255) as u8;
        }
        buf
    }

    /// OK surface with a centered bright defect patch (~235 DN).
    pub fn defect_surface(seed: u64) -> Vec<u8> {
        let mut buf = ok_surface(seed);
        let cx = (STRIDE / 2) as usize;
        let cy = (HEIGHT / 2) as usize;
        for dy in 0..8 {
            for dx in 0..8 {
                let x = cx + dx;
                let y = cy + dy;
                if x < STRIDE as usize && y < HEIGHT as usize {
                    buf[y * STRIDE as usize + x] = 235;
                }
            }
        }
        buf
    }

    /// Apply illumination gain + offset (saturating) — for ablation studies.
    pub fn apply_illumination(src: &[u8], gain: f32, offset: f32) -> Vec<u8> {
        src.iter()
            .map(|&p| {
                let v = (p as f32) * gain + offset;
                v.clamp(0.0, 255.0) as u8
            })
            .collect()
    }
}

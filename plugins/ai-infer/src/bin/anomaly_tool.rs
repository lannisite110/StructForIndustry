//! sfi-anomaly — OK-only model calibration + reports (changeover / latency /
//! illumination / error-rates).
//!
//! Subcommands:
//!   calibrate --ok N --out model.json      Calibrate from N synthetic OK frames
//!   score --model model.json [--defect]    Score one OK/defect frame
//!   dump --kind ok|ng --out frame.gray8    Write a raw Gray8 frame for replay
//!   report changeover|latency|illum|errors|all   Emit a markdown report

use std::path::PathBuf;
use std::time::Instant;

use sfi_ai_infer::anomaly::{calibrate, AnomalyModel, CalibrateConfig, Extractor};
use sfi_ai_infer::synthetic::{
    apply_illumination, defect_at, defect_surface, ok_surface, ok_surface_amp, HEIGHT, WIDTH,
};

fn ok_frames(n: u32) -> Vec<(Vec<u8>, u32, u32)> {
    (0..n)
        .map(|i| (ok_surface(i as u64), WIDTH, HEIGHT))
        .collect()
}

/// Parse `--extractor handcrafted|onnx[:model.onnx]` (default onnx reference).
fn extractor_from_args(args: &[String]) -> Extractor {
    match arg_value(args, "--extractor").as_deref() {
        Some("handcrafted") => Extractor::Handcrafted,
        Some(s) if s.starts_with("onnx") => {
            let model = s.strip_prefix("onnx:").unwrap_or("").to_string();
            Extractor::Onnx { model }
        }
        _ => Extractor::Onnx {
            model: String::new(),
        },
    }
}

fn config_from_args(args: &[String]) -> CalibrateConfig {
    CalibrateConfig {
        extractor: extractor_from_args(args),
        ..Default::default()
    }
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn has_flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

fn cmd_calibrate(args: &[String]) -> Result<(), String> {
    let n: u32 = arg_value(args, "--ok")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let out = arg_value(args, "--out").map(PathBuf::from);
    let cfg = config_from_args(args);
    let model = calibrate(&ok_frames(n), &cfg)?;
    let json = model.to_json()?;
    match out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
            }
            std::fs::write(&path, &json).map_err(|e| e.to_string())?;
            eprintln!(
                "calibrated on {n} OK frames ({}, dim={}) -> {} (threshold={:.5}, ok_max={:.5})",
                extractor_label(&model.extractor),
                model.descriptor_dim,
                path.display(),
                model.threshold,
                model.ok_score_max
            );
        }
        None => println!("{json}"),
    }
    Ok(())
}

fn extractor_label(e: &Extractor) -> String {
    match e {
        Extractor::Handcrafted => "handcrafted".into(),
        Extractor::Onnx { model } if model.is_empty() => "onnx-ref (filter-bank)".into(),
        Extractor::Onnx { model } => format!("onnx ({model})"),
    }
}

fn frame_of_kind(defect: bool) -> Vec<u8> {
    if defect {
        defect_surface(0)
    } else {
        ok_surface(777)
    }
}

fn cmd_dump(args: &[String]) -> Result<(), String> {
    let out = PathBuf::from(arg_value(args, "--out").ok_or("missing --out")?);
    let defect = arg_value(args, "--kind").as_deref() == Some("ng");
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(&out, frame_of_kind(defect)).map_err(|e| e.to_string())?;
    eprintln!(
        "wrote {} frame {WIDTH}x{HEIGHT} -> {}",
        if defect { "NG" } else { "OK" },
        out.display()
    );
    Ok(())
}

fn cmd_score(args: &[String]) -> Result<(), String> {
    let model_path = arg_value(args, "--model").ok_or("missing --model")?;
    let model = AnomalyModel::load(&PathBuf::from(model_path))?;
    let frame = frame_of_kind(has_flag(args, "--defect"));
    let r = model.score(&frame, WIDTH, HEIGHT).ok_or("score failed")?;
    println!(
        "score={:.5} threshold={:.5} verdict={} worst_cell=({},{})",
        r.score,
        r.threshold,
        if r.is_defect() { "NG" } else { "OK" },
        r.worst_col,
        r.worst_row
    );
    Ok(())
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1);
    sorted[idx.min(sorted.len() - 1)]
}

fn report_changeover(cfg: &CalibrateConfig) -> String {
    let mut out = String::from("## Changeover curve (OK samples vs separation)\n\n");
    out.push_str(&format!(
        "Feature extractor: **{}**. How detection margin grows with more OK samples.\n\n",
        extractor_label(&cfg.extractor)
    ));
    out.push_str("| OK samples | threshold | defect score | margin (score/thr) | verdict |\n");
    out.push_str("|-----------:|----------:|-------------:|-------------------:|:-------:|\n");
    let defect = defect_surface(0);
    for &n in &[1u32, 5, 10, 20] {
        let model = match calibrate(&ok_frames(n), cfg) {
            Ok(m) => m,
            Err(e) => {
                out.push_str(&format!("| {n} | error: {e} | | | |\n"));
                continue;
            }
        };
        let r = model.score(&defect, WIDTH, HEIGHT).unwrap();
        let margin = if r.threshold > 0.0 {
            r.score / r.threshold
        } else {
            0.0
        };
        out.push_str(&format!(
            "| {n} | {:.5} | {:.5} | {:.2}x | {} |\n",
            r.threshold,
            r.score,
            margin,
            if r.is_defect() { "NG ✓" } else { "OK ✗" }
        ));
    }
    out.push('\n');
    out
}

fn report_latency(cfg: &CalibrateConfig) -> String {
    let mut out = String::from("## Inference latency (CPU, anomaly scorer)\n\n");
    out.push_str(&format!(
        "Per-frame anomaly scoring latency (single thread). Feature extractor: **{}**.\n\n",
        extractor_label(&cfg.extractor)
    ));
    out.push_str("| resolution | iters | p50 | p95 | p99 | mean |\n");
    out.push_str("|-----------|------:|----:|----:|----:|-----:|\n");

    let model = calibrate(&ok_frames(20), cfg).unwrap();
    // Score at the calibration resolution; larger resolutions reuse the same
    // grid so cost scales with pixels.
    for &(w, h, iters) in &[(WIDTH, HEIGHT, 2000u32), (640, 480, 500), (1920, 1080, 200)] {
        let frame = synth_resized(w, h);
        // warmup
        for _ in 0..10 {
            let _ = model.score(&frame, w, h);
        }
        let mut samples = Vec::with_capacity(iters as usize);
        for _ in 0..iters {
            let t = Instant::now();
            let _ = model.score(&frame, w, h);
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        out.push_str(&format!(
            "| {w}x{h} | {iters} | {:.3}ms | {:.3}ms | {:.3}ms | {:.3}ms |\n",
            percentile(&samples, 0.50),
            percentile(&samples, 0.95),
            percentile(&samples, 0.99),
            mean
        ));
    }
    out.push_str(
        "\n> Budget: <20ms/frame at 1080p. Small/VGA frames clear it with margin on a \
single CPU thread; richer (onnx-ref) features at full HD sit near the budget — for HD \
real-time use the ONNX GPU execution provider, per-cell parallelism, or ROI tiling.\n\n",
    );
    out
}

/// Tile the 64x48 synthetic pattern up to an arbitrary resolution.
fn synth_resized(w: u32, h: u32) -> Vec<u8> {
    let base = defect_surface(0);
    let mut out = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let bx = x % WIDTH;
            let by = y % HEIGHT;
            out[(y * w + x) as usize] = base[(by * WIDTH + bx) as usize];
        }
    }
    out
}

fn report_illum(cfg: &CalibrateConfig) -> String {
    let mut out = String::from("## Illumination ablation (robustness)\n\n");
    out.push_str(&format!(
        "Apply gain/offset to OK and defect frames after calibrating on nominal OK light. \
A robust model keeps OK below and defect above threshold across lighting changes. \
Feature extractor: **{}**.\n\n",
        extractor_label(&cfg.extractor)
    ));
    out.push_str("| gain | offset | OK score | OK verdict | defect score | defect verdict |\n");
    out.push_str("|-----:|-------:|---------:|:----------:|-------------:|:--------------:|\n");

    let model = calibrate(&ok_frames(20), cfg).unwrap();
    let ok_base = ok_surface(123);
    let ng_base = defect_surface(0);
    for &(gain, offset) in &[
        (1.0f32, 0.0f32),
        (1.0, -30.0),
        (1.0, 30.0),
        (0.8, 0.0),
        (1.2, 0.0),
        (1.2, 20.0),
    ] {
        let ok = apply_illumination(&ok_base, gain, offset);
        let ng = apply_illumination(&ng_base, gain, offset);
        let ok_r = model.score(&ok, WIDTH, HEIGHT).unwrap();
        let ng_r = model.score(&ng, WIDTH, HEIGHT).unwrap();
        out.push_str(&format!(
            "| {:.1} | {:+.0} | {:.5} | {} | {:.5} | {} |\n",
            gain,
            offset,
            ok_r.score,
            if ok_r.is_defect() { "NG ✗" } else { "OK ✓" },
            ng_r.score,
            if ng_r.is_defect() { "NG ✓" } else { "OK ✗" }
        ));
    }
    out.push_str(&format!(
        "\n> illumination normalization: **{}**\n\n",
        if model.normalize_illumination {
            "on"
        } else {
            "off"
        }
    ));
    out
}

/// Labeled synthetic test set: `(frame, is_defect)`.
///
/// Deliberately includes hard cases on both sides so the trade-off is real:
/// OK frames range from nominal (±3 DN, as calibrated) to noisier than the
/// calibration set saw (up to ±10 DN), and defects span from near the noise
/// floor (tiny, ~10 DN contrast) to strong, across positions.
fn labeled_test_set() -> Vec<(Vec<u8>, bool)> {
    let mut set = Vec::new();
    // 40 nominal OK + 20 noisier-than-calibration OK.
    for seed in 2000..2040u64 {
        set.push((ok_surface(seed), false));
    }
    for (i, seed) in (2040..2060u64).enumerate() {
        let amp = 5 + (i as i32 % 6); // 5..10 DN
        set.push((ok_surface_amp(seed, amp), false));
    }
    // Defects: contrast over a ~108..128 background; 130/140 are near noise.
    let values = [130u8, 140, 160, 190, 235];
    let sizes = [2usize, 3, 5, 8];
    let positions = [(12usize, 8usize), (28, 16), (44, 28), (20, 32), (50, 10)];
    let mut seed = 3000u64;
    for &value in &values {
        for &size in &sizes {
            for &(cx, cy) in &positions {
                set.push((defect_at(seed, cx, cy, size, value), true));
                seed += 1;
            }
        }
    }
    set
}

struct Confusion {
    tp: u32,
    fp: u32,
    tn: u32,
    fn_: u32,
}

impl Confusion {
    fn tally(scored: &[(f32, bool)], threshold: f32) -> Self {
        let mut c = Confusion {
            tp: 0,
            fp: 0,
            tn: 0,
            fn_: 0,
        };
        for &(score, is_defect) in scored {
            let flagged = score > threshold;
            match (is_defect, flagged) {
                (true, true) => c.tp += 1,
                (true, false) => c.fn_ += 1,
                (false, true) => c.fp += 1,
                (false, false) => c.tn += 1,
            }
        }
        c
    }
    fn fpr(&self) -> f32 {
        let d = self.fp + self.tn;
        if d == 0 {
            0.0
        } else {
            self.fp as f32 / d as f32
        }
    }
    fn miss_rate(&self) -> f32 {
        let d = self.tp + self.fn_;
        if d == 0 {
            0.0
        } else {
            self.fn_ as f32 / d as f32
        }
    }
    fn precision(&self) -> f32 {
        let d = self.tp + self.fp;
        if d == 0 {
            0.0
        } else {
            self.tp as f32 / d as f32
        }
    }
    fn recall(&self) -> f32 {
        1.0 - self.miss_rate()
    }
}

fn report_errors(cfg: &CalibrateConfig) -> String {
    let model = calibrate(&ok_frames(20), cfg).unwrap();
    let test = labeled_test_set();
    let n_ok = test.iter().filter(|(_, d)| !*d).count();
    let n_ng = test.len() - n_ok;
    let scored: Vec<(f32, bool)> = test
        .iter()
        .map(|(f, d)| (model.score(f, WIDTH, HEIGHT).unwrap().score, *d))
        .collect();

    let mut out = String::from("## False-positive / miss rate\n\n");
    out.push_str(&format!(
        "Feature extractor: **{}**. Test set: {n_ok} OK ({} nominal + {} noisier than \
calibration) + {n_ng} defect frames (contrast 130–235 DN, size 2–8 px, varied position). \
Calibrated on 20 nominal OK frames only.\n\n",
        extractor_label(&cfg.extractor),
        n_ok - 20,
        20
    ));

    // Operating point at the calibrated threshold.
    let c = Confusion::tally(&scored, model.threshold);
    out.push_str("**Operating point (calibrated threshold)**\n\n");
    out.push_str("| threshold | TP | FP | TN | FN | FPR (误报率) | miss rate (漏检率) | precision | recall |\n");
    out.push_str("|----------:|---:|---:|---:|---:|-----------:|-------------------:|----------:|-------:|\n");
    out.push_str(&format!(
        "| {:.4} | {} | {} | {} | {} | {:.1}% | {:.1}% | {:.1}% | {:.1}% |\n\n",
        model.threshold,
        c.tp,
        c.fp,
        c.tn,
        c.fn_,
        c.fpr() * 100.0,
        c.miss_rate() * 100.0,
        c.precision() * 100.0,
        c.recall() * 100.0
    ));

    // Threshold sweep — FPR vs miss-rate trade-off.
    out.push_str("**Threshold sweep (FPR vs miss-rate trade-off)**\n\n");
    out.push_str("| threshold scale | threshold | FPR (误报率) | miss rate (漏检率) |\n");
    out.push_str("|----------------:|----------:|-----------:|-------------------:|\n");
    for &scale in &[0.7f32, 0.85, 1.0, 1.2, 1.5, 2.0] {
        let thr = model.threshold * scale;
        let c = Confusion::tally(&scored, thr);
        out.push_str(&format!(
            "| {:.2}x | {:.4} | {:.1}% | {:.1}% |\n",
            scale,
            thr,
            c.fpr() * 100.0,
            c.miss_rate() * 100.0
        ));
    }
    out.push_str(
        "\n> Lower threshold → fewer escapes (miss), more false alarms; higher → the \
reverse. The noisier-than-calibration OK frames drive most false positives, so the \
real lever is recalibrating on representative OK frames (and tuning the margin) to the \
line's target miss-rate.\n\n",
    );
    out
}

fn cmd_report(args: &[String]) -> Result<(), String> {
    let kind = args.first().map(String::as_str).unwrap_or("all");
    let cfg = config_from_args(args);
    let mut out = String::from("# Anomaly detection reports\n\n");
    out.push_str(
        "_Generated by `sfi-anomaly report` (synthetic frames; replace with bench-rig data)._\n\n",
    );
    match kind {
        "changeover" => out.push_str(&report_changeover(&cfg)),
        "latency" => out.push_str(&report_latency(&cfg)),
        "illum" => out.push_str(&report_illum(&cfg)),
        "errors" => out.push_str(&report_errors(&cfg)),
        "all" => {
            out.push_str(&report_changeover(&cfg));
            out.push_str(&report_latency(&cfg));
            out.push_str(&report_illum(&cfg));
            out.push_str(&report_errors(&cfg));
        }
        other => return Err(format!("unknown report: {other}")),
    }
    print!("{out}");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().cloned().unwrap_or_default();
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };
    let result = match cmd.as_str() {
        "calibrate" => cmd_calibrate(rest),
        "score" => cmd_score(rest),
        "dump" => cmd_dump(rest),
        "report" => cmd_report(rest),
        "" | "-h" | "--help" => {
            eprintln!(
                "usage: sfi-anomaly <calibrate|score|dump|report> ...\n\
                 calibrate --ok N --out model.json [--extractor handcrafted|onnx[:model.onnx]]\n\
                 score --model model.json [--defect]\n\
                 dump --kind ok|ng --out frame.gray8\n\
                 report changeover|latency|illum|errors|all [--extractor handcrafted|onnx[:model.onnx]]"
            );
            return;
        }
        other => Err(format!("unknown command: {other}")),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confusion_rates() {
        // 2 defects (scores 0.9, 0.3), 2 OK (0.4, 0.1); threshold 0.5.
        let scored = [(0.9, true), (0.3, true), (0.4, false), (0.1, false)];
        let c = Confusion::tally(&scored, 0.5);
        assert_eq!((c.tp, c.fp, c.tn, c.fn_), (1, 0, 2, 1));
        assert_eq!(c.miss_rate(), 0.5); // one defect (0.3) missed
        assert_eq!(c.fpr(), 0.0);
        assert_eq!(c.recall(), 0.5);
    }

    #[test]
    fn test_set_is_balanced_and_labeled() {
        let set = labeled_test_set();
        let ok = set.iter().filter(|(_, d)| !*d).count();
        let ng = set.len() - ok;
        assert_eq!(ok, 60);
        assert_eq!(ng, 100);
    }

    #[test]
    fn calibrated_threshold_catches_most_defects() {
        let model = calibrate(&ok_frames(20), &config_from_args(&[])).unwrap();
        let scored: Vec<(f32, bool)> = labeled_test_set()
            .iter()
            .map(|(f, d)| (model.score(f, WIDTH, HEIGHT).unwrap().score, *d))
            .collect();
        let c = Confusion::tally(&scored, model.threshold);
        assert!(c.recall() > 0.9, "recall too low: {}", c.recall());
    }
}

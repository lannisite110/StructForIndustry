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
use sfi_ai_infer::bench_fixtures::{
    bench_root_exists, default_bench_root, default_frame_dims, load_defect_frames, load_ok_frames,
    write_synthetic_bench_tree,
};
use sfi_ai_infer::synthetic::{
    apply_illumination, defect_at, defect_surface, ok_surface, ok_surface_amp, HEIGHT, WIDTH,
};

struct FrameCtx {
    width: u32,
    height: u32,
    fixture_root: Option<PathBuf>,
    from_dir: Option<PathBuf>,
}

impl FrameCtx {
    fn from_args(args: &[String]) -> Self {
        let (dw, dh) = default_frame_dims();
        let width = arg_value(args, "--width")
            .and_then(|v| v.parse().ok())
            .or_else(|| std::env::var("SFI_BENCH_WIDTH").ok().and_then(|v| v.parse().ok()))
            .unwrap_or(dw);
        let height = arg_value(args, "--height")
            .and_then(|v| v.parse().ok())
            .or_else(|| std::env::var("SFI_BENCH_HEIGHT").ok().and_then(|v| v.parse().ok()))
            .unwrap_or(dh);
        let fixture_root = arg_value(args, "--fixture-root").map(PathBuf::from);
        let from_dir = arg_value(args, "--from-dir").map(PathBuf::from);
        Self {
            width,
            height,
            fixture_root,
            from_dir,
        }
    }

    fn ok_frames(&self, n: u32) -> Result<Vec<(Vec<u8>, u32, u32)>, String> {
        if let Some(dir) = &self.from_dir {
            return sfi_ai_infer::bench_fixtures::load_gray8_dir(dir, self.width, self.height)
                .map_err(|e| e.to_string())
                .map(|frames| {
                    frames
                        .into_iter()
                        .map(|f| (f.pixels, f.width, f.height))
                        .collect()
                });
        }
        if let Some(root) = &self.fixture_root {
            let all = load_ok_frames(root, self.width, self.height).map_err(|e| e.to_string())?;
            if !all.is_empty() {
                if n as usize >= all.len() {
                    return Ok(all);
                }
                return Ok(all.into_iter().take(n as usize).collect());
            }
        }
        Ok((0..n)
            .map(|i| (ok_surface(i as u64), self.width, self.height))
            .collect())
    }

    fn data_source_label(&self) -> String {
        if self.from_dir.is_some() {
            format!("bench dir `{}`", self.from_dir.as_ref().unwrap().display())
        } else if self.fixture_root.is_some() {
            format!(
                "bench fixtures `{}`",
                self.fixture_root.as_ref().unwrap().display()
            )
        } else {
            "synthetic frames".into()
        }
    }

    fn reference_defect(&self) -> Vec<u8> {
        if let Some(root) = &self.fixture_root {
            if let Ok(defects) = load_defect_frames(root, self.width, self.height) {
                if let Some((f, _, _)) = defects.first() {
                    return f.clone();
                }
            }
        }
        defect_surface(0)
    }
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
    let ctx = FrameCtx::from_args(args);
    let frames = ctx.ok_frames(n)?;
    let model = calibrate(&frames, &cfg)?;
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

fn report_changeover(cfg: &CalibrateConfig, ctx: &FrameCtx) -> String {
    let mut out = String::from("## Changeover curve (OK samples vs separation)\n\n");
    out.push_str(&format!(
        "Feature extractor: **{}**. How detection margin grows with more OK samples.\n\n",
        extractor_label(&cfg.extractor)
    ));
    out.push_str("| OK samples | threshold | defect score | margin (score/thr) | verdict |\n");
    out.push_str("|-----------:|----------:|-------------:|-------------------:|:-------:|\n");
    let defect = ctx.reference_defect();
    for &n in &[1u32, 5, 10, 20] {
        let model = match ctx.ok_frames(n).and_then(|ok| calibrate(&ok, cfg).map_err(|e| e.to_string())) {
            Ok(m) => m,
            Err(e) => {
                out.push_str(&format!("| {n} | error: {e} | | | |\n"));
                continue;
            }
        };
        let r = model.score(&defect, ctx.width, ctx.height).unwrap();
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

fn report_latency(cfg: &CalibrateConfig, ctx: &FrameCtx) -> String {
    let mut out = String::from("## Inference latency (CPU, anomaly scorer)\n\n");
    out.push_str(&format!(
        "Per-frame anomaly scoring latency (single thread). Feature extractor: **{}**.\n\n",
        extractor_label(&cfg.extractor)
    ));
    out.push_str("| resolution | iters | p50 | p95 | p99 | mean |\n");
    out.push_str("|-----------|------:|----:|----:|----:|-----:|\n");

    let model = calibrate(&ctx.ok_frames(20).unwrap(), cfg).unwrap();
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

fn report_illum(cfg: &CalibrateConfig, ctx: &FrameCtx) -> String {
    let mut out = String::from("## Illumination ablation (robustness)\n\n");
    out.push_str(&format!(
        "Apply gain/offset to OK and defect frames after calibrating on nominal OK light. \
A robust model keeps OK below and defect above threshold across lighting changes. \
Feature extractor: **{}**.\n\n",
        extractor_label(&cfg.extractor)
    ));
    out.push_str("| gain | offset | OK score | OK verdict | defect score | defect verdict |\n");
    out.push_str("|-----:|-------:|---------:|:----------:|-------------:|:--------------:|\n");

    let model = calibrate(&ctx.ok_frames(20).unwrap(), cfg).unwrap();
    let ok_base = if let Ok(ok) = ctx.ok_frames(1) {
        ok[0].0.clone()
    } else {
        ok_surface(123)
    };
    let ng_base = ctx.reference_defect();
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
        let ok_r = model.score(&ok, ctx.width, ctx.height).unwrap();
        let ng_r = model.score(&ng, ctx.width, ctx.height).unwrap();
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
fn labeled_test_set_synthetic() -> Vec<(Vec<u8>, bool)> {
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

fn labeled_test_set(ctx: &FrameCtx) -> Vec<(Vec<u8>, bool)> {
    if let Some(root) = &ctx.fixture_root {
        let mut set = Vec::new();
        if let Ok(ok) = load_ok_frames(root, ctx.width, ctx.height) {
            for (f, _, _) in ok {
                set.push((f, false));
            }
        }
        if let Ok(def) = load_defect_frames(root, ctx.width, ctx.height) {
            for (f, _, _) in def {
                set.push((f, true));
            }
        }
        if !set.is_empty() {
            return set;
        }
    }
    labeled_test_set_synthetic()
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

fn report_errors(cfg: &CalibrateConfig, ctx: &FrameCtx) -> String {
    let model = calibrate(&ctx.ok_frames(20).unwrap(), cfg).unwrap();
    let test = labeled_test_set(ctx);
    let n_ok = test.iter().filter(|(_, d)| !*d).count();
    let n_ng = test.len() - n_ok;
    let scored: Vec<(f32, bool)> = test
        .iter()
        .map(|(f, d)| (model.score(f, ctx.width, ctx.height).unwrap().score, *d))
        .collect();

    let mut out = String::from("## False-positive / miss rate\n\n");
    out.push_str(&format!(
        "Feature extractor: **{}**. Test set: {n_ok} OK + {n_ng} defect frames. \
Calibrated on 20 OK frames from {}.\n\n",
        extractor_label(&cfg.extractor),
        ctx.data_source_label()
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
    let mut ctx = FrameCtx::from_args(args);
    if ctx.fixture_root.is_none() {
        let root = default_bench_root();
        if bench_root_exists(&root) {
            ctx.fixture_root = Some(root);
        }
    }
    let mut out = String::from("# Anomaly detection reports\n\n");
    out.push_str(&format!(
        "_Generated by `sfi-anomaly report`. Data source: **{}**._\n\n",
        ctx.data_source_label()
    ));
    match kind {
        "changeover" => out.push_str(&report_changeover(&cfg, &ctx)),
        "latency" => out.push_str(&report_latency(&cfg, &ctx)),
        "illum" => out.push_str(&report_illum(&cfg, &ctx)),
        "errors" => out.push_str(&report_errors(&cfg, &ctx)),
        "all" => {
            out.push_str(&report_changeover(&cfg, &ctx));
            out.push_str(&report_latency(&cfg, &ctx));
            out.push_str(&report_illum(&cfg, &ctx));
            out.push_str(&report_errors(&cfg, &ctx));
        }
        other => return Err(format!("unknown report: {other}")),
    }
    print!("{out}");
    Ok(())
}

fn cmd_gen_fixtures(args: &[String]) -> Result<(), String> {
    let root = arg_value(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(default_bench_root);
    write_synthetic_bench_tree(&root).map_err(|e| e.to_string())?;
    eprintln!("wrote bench fixtures -> {}", root.display());
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
        "gen-fixtures" => cmd_gen_fixtures(rest),
        "" | "-h" | "--help" => {
            eprintln!(
                "usage: sfi-anomaly <calibrate|score|dump|report|gen-fixtures> ...\n\
                 calibrate --ok N --out model.json [--from-dir DIR] [--fixture-root ROOT] [--width W --height H]\n\
                 score --model model.json [--defect]\n\
                 dump --kind ok|ng --out frame.gray8\n\
                 report changeover|latency|illum|errors|all [--fixture-root ROOT]\n\
                 gen-fixtures [--out tools/fixtures/bench]"
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
        let set = labeled_test_set_synthetic();
        let ok = set.iter().filter(|(_, d)| !*d).count();
        let ng = set.len() - ok;
        assert_eq!(ok, 60);
        assert_eq!(ng, 100);
    }

    #[test]
    fn calibrated_threshold_catches_most_defects() {
        let ctx = FrameCtx::from_args(&[]);
        let model = calibrate(&ctx.ok_frames(20).unwrap(), &config_from_args(&[])).unwrap();
        let scored: Vec<(f32, bool)> = labeled_test_set_synthetic()
            .iter()
            .map(|(f, d)| (model.score(f, WIDTH, HEIGHT).unwrap().score, *d))
            .collect();
        let c = Confusion::tally(&scored, model.threshold);
        assert!(c.recall() > 0.9, "recall too low: {}", c.recall());
    }
}

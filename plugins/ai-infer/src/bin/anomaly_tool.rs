//! sfi-anomaly — OK-only model calibration + the three reports.
//!
//! Subcommands:
//!   calibrate --ok N --out model.json      Calibrate from N synthetic OK frames
//!   score --model model.json [--defect]    Score one OK/defect frame
//!   report changeover|latency|illum|all    Emit a markdown report to stdout

use std::path::PathBuf;
use std::time::Instant;

use sfi_ai_infer::anomaly::{calibrate, AnomalyModel, CalibrateConfig};
use sfi_ai_infer::synthetic::{apply_illumination, defect_surface, ok_surface, HEIGHT, WIDTH};

fn ok_frames(n: u32) -> Vec<(Vec<u8>, u32, u32)> {
    (0..n)
        .map(|i| (ok_surface(i as u64), WIDTH, HEIGHT))
        .collect()
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
    let cfg = CalibrateConfig::default();
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
                "calibrated on {n} OK frames -> {} (threshold={:.5}, ok_max={:.5})",
                path.display(),
                model.threshold,
                model.ok_score_max
            );
        }
        None => println!("{json}"),
    }
    Ok(())
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

fn report_changeover() -> String {
    let mut out = String::from("## Changeover curve (OK samples vs separation)\n\n");
    out.push_str("How detection margin grows as more OK samples are used to calibrate.\n\n");
    out.push_str("| OK samples | threshold | defect score | margin (score/thr) | verdict |\n");
    out.push_str("|-----------:|----------:|-------------:|-------------------:|:-------:|\n");
    let defect = defect_surface(0);
    for &n in &[1u32, 5, 10, 20] {
        let model = match calibrate(&ok_frames(n), &CalibrateConfig::default()) {
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

fn report_latency() -> String {
    let mut out = String::from("## Inference latency (CPU, anomaly scorer)\n\n");
    out.push_str("Per-frame anomaly scoring latency on synthetic frames (single thread).\n\n");
    out.push_str("| resolution | iters | p50 | p95 | p99 | mean |\n");
    out.push_str("|-----------|------:|----:|----:|----:|-----:|\n");

    let model = calibrate(&ok_frames(20), &CalibrateConfig::default()).unwrap();
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
    out.push_str("\n> Target hardware budget: <20ms/frame for 1080p inference.\n\n");
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

fn report_illum() -> String {
    let mut out = String::from("## Illumination ablation (robustness)\n\n");
    out.push_str(
        "Apply gain/offset to OK and defect frames after calibrating on nominal OK light. \
A robust model keeps OK below and defect above threshold across lighting changes.\n\n",
    );
    out.push_str("| gain | offset | OK score | OK verdict | defect score | defect verdict |\n");
    out.push_str("|-----:|-------:|---------:|:----------:|-------------:|:--------------:|\n");

    let model = calibrate(&ok_frames(20), &CalibrateConfig::default()).unwrap();
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

fn cmd_report(args: &[String]) -> Result<(), String> {
    let kind = args.first().map(String::as_str).unwrap_or("all");
    let mut out = String::from("# Anomaly detection reports\n\n");
    out.push_str(
        "_Generated by `sfi-anomaly report` (synthetic frames; replace with bench-rig data)._\n\n",
    );
    match kind {
        "changeover" => out.push_str(&report_changeover()),
        "latency" => out.push_str(&report_latency()),
        "illum" => out.push_str(&report_illum()),
        "all" => {
            out.push_str(&report_changeover());
            out.push_str(&report_latency());
            out.push_str(&report_illum());
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
                 calibrate --ok N --out model.json\n\
                 score --model model.json [--defect]\n\
                 dump --kind ok|ng --out frame.gray8\n\
                 report changeover|latency|illum|all"
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

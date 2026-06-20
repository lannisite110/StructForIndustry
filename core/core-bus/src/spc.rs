//! SPC rolling metrics — published on topic `spc.metrics`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use capnp::message::Builder;
use sfi_contracts::result_capnp;
use sfi_plugin_host::TaskResponse;

use crate::profile::SpcLimitsSection;

const MEASURE_METRIC_UNITS: &[(&str, &str)] = &[
    ("edge_position_px", "px"),
    ("edge_position_mm", "mm"),
    ("edge_deviation_px", "px"),
    ("edge_deviation_mm", "mm"),
    ("edge_strength", "dn"),
    ("line_width_px", "px"),
    ("line_width_mm", "mm"),
    ("circle_diameter_px", "px"),
    ("circle_diameter_mm", "mm"),
    ("circle_radius_px", "px"),
    ("dimension_deviation_px", "px"),
    ("dimension_deviation_mm", "mm"),
    ("ncc_score", "ratio"),
    ("template_offset_x_px", "px"),
    ("template_offset_y_px", "px"),
    ("position_deviation_x_px", "px"),
    ("position_deviation_y_px", "px"),
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SpcMetricValue {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SpcSnapshot {
    pub frame_id: u64,
    pub published_at_ns: u64,
    pub values: Vec<SpcMetricValue>,
}

#[derive(Clone)]
pub struct SpcEngine {
    inner: Arc<Mutex<SpcState>>,
}

struct SpcState {
    window: usize,
    limits: SpcLimitsSection,
    histogram_bins: usize,
    gray_samples: VecDeque<f64>,
    ng_samples: VecDeque<bool>,
    last: Option<SpcSnapshot>,
}

impl SpcEngine {
    pub fn new(window: usize) -> Self {
        Self::with_config(window, SpcLimitsSection::default(), 16)
    }

    pub fn with_config(window: usize, limits: SpcLimitsSection, histogram_bins: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SpcState {
                window: window.max(1),
                limits,
                histogram_bins: histogram_bins.clamp(4, 64),
                gray_samples: VecDeque::new(),
                ng_samples: VecDeque::new(),
                last: None,
            })),
        }
    }

    pub fn ingest(&self, frame_id: u64, resp: &TaskResponse, published_at_ns: u64) -> SpcSnapshot {
        let gray_mean = metric_value(resp, "gray_mean")
            .unwrap_or_else(|| metric_value(resp, "bright_pixels").unwrap_or(0.0));
        let is_ng = resp.status == "error"
            || resp.detections.iter().any(|d| {
                if d.class_id == 10 || d.class_id == 11 || d.class_id == 12 {
                    return false;
                }
                d.class_id == 1
                    || d.class_id == 99
                    || d.label == "surface_defect"
                    || d.label == "defect"
            });

        let mut state = self.inner.lock().expect("spc lock");
        let window = state.window;
        push_window(&mut state.gray_samples, gray_mean, window);
        push_window(&mut state.ng_samples, is_ng, window);

        let gray_rolling = mean(state.gray_samples.iter().copied());
        let gray_std = std_dev(state.gray_samples.iter().copied());
        let gray_min = min_val(state.gray_samples.iter().copied());
        let gray_max = max_val(state.gray_samples.iter().copied());
        let ng_rate = if state.ng_samples.is_empty() {
            0.0
        } else {
            state.ng_samples.iter().filter(|&&ng| ng).count() as f64 / state.ng_samples.len() as f64
        };

        let defect_components = metric_value(resp, "defect_components").unwrap_or(0.0);
        let (cp, cpk) = cp_cpk(gray_rolling, gray_std, state.limits.usl, state.limits.lsl);
        let (hist_peak_dn, hist_peak_ratio) =
            histogram_peak(&state.gray_samples, state.histogram_bins);

        let mut values = vec![
            SpcMetricValue {
                name: "gray_mean".into(),
                value: gray_mean,
                unit: "dn".into(),
            },
            SpcMetricValue {
                name: "gray_mean_rolling".into(),
                value: gray_rolling,
                unit: "dn".into(),
            },
            SpcMetricValue {
                name: "gray_mean_std".into(),
                value: gray_std,
                unit: "dn".into(),
            },
            SpcMetricValue {
                name: "gray_mean_min".into(),
                value: gray_min,
                unit: "dn".into(),
            },
            SpcMetricValue {
                name: "gray_mean_max".into(),
                value: gray_max,
                unit: "dn".into(),
            },
            SpcMetricValue {
                name: "ng_rate".into(),
                value: ng_rate,
                unit: "ratio".into(),
            },
            SpcMetricValue {
                name: "defect_components".into(),
                value: defect_components,
                unit: "count".into(),
            },
            SpcMetricValue {
                name: "gray_hist_peak_dn".into(),
                value: hist_peak_dn,
                unit: "dn".into(),
            },
            SpcMetricValue {
                name: "gray_hist_peak_ratio".into(),
                value: hist_peak_ratio,
                unit: "ratio".into(),
            },
        ];
        if cp.is_finite() {
            values.push(SpcMetricValue {
                name: "cp".into(),
                value: cp,
                unit: "index".into(),
            });
        }
        if cpk.is_finite() {
            values.push(SpcMetricValue {
                name: "cpk".into(),
                value: cpk,
                unit: "index".into(),
            });
        }

        for (name, unit) in MEASURE_METRIC_UNITS {
            if let Some(v) = metric_value(resp, name) {
                values.push(SpcMetricValue {
                    name: (*name).into(),
                    value: v,
                    unit: (*unit).into(),
                });
            }
        }

        let snapshot = SpcSnapshot {
            frame_id,
            published_at_ns,
            values,
        };
        state.last = Some(snapshot.clone());
        snapshot
    }

    pub fn last(&self) -> Option<SpcSnapshot> {
        self.inner.lock().expect("spc lock").last.clone()
    }
}

impl Default for SpcEngine {
    fn default() -> Self {
        Self::new(32)
    }
}

pub fn metrics_payload_bytes(snapshot: &SpcSnapshot) -> Result<Vec<u8>, capnp::Error> {
    let mut message = Builder::new_default();
    let mut metrics = message.init_root::<result_capnp::metrics_payload::Builder>();
    metrics.set_frame_id(snapshot.frame_id);
    let mut vals = metrics.init_values(snapshot.values.len() as u32);
    for (i, mv) in snapshot.values.iter().enumerate() {
        let mut b = vals.reborrow().get(i as u32);
        b.set_name(&mv.name);
        b.set_value(mv.value);
        b.set_unit(&mv.unit);
    }
    let mut bytes = Vec::new();
    capnp::serialize::write_message(&mut bytes, &message)?;
    Ok(bytes)
}

fn metric_value(resp: &TaskResponse, name: &str) -> Option<f64> {
    resp.metrics
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.value)
}

fn push_window<T>(q: &mut VecDeque<T>, v: T, cap: usize) {
    q.push_back(v);
    while q.len() > cap {
        q.pop_front();
    }
}

fn mean(xs: impl Iterator<Item = f64>) -> f64 {
    let mut n = 0usize;
    let mut sum = 0.0;
    for x in xs {
        n += 1;
        sum += x;
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f64
    }
}

fn std_dev(xs: impl Iterator<Item = f64>) -> f64 {
    let mut n = 0usize;
    let mut sum = 0.0;
    let mut sum2 = 0.0;
    for x in xs {
        n += 1;
        sum += x;
        sum2 += x * x;
    }
    if n < 2 {
        return 0.0;
    }
    let μ = sum / n as f64;
    let var = sum2 / n as f64 - μ * μ;
    var.max(0.0).sqrt()
}

fn min_val(xs: impl Iterator<Item = f64>) -> f64 {
    xs.fold(f64::INFINITY, |a, b| a.min(b))
}

fn max_val(xs: impl Iterator<Item = f64>) -> f64 {
    xs.fold(f64::NEG_INFINITY, |a, b| a.max(b))
}

fn cp_cpk(mean: f64, sigma: f64, usl: f64, lsl: f64) -> (f64, f64) {
    if usl <= lsl || sigma < 1e-9 {
        return (f64::NAN, f64::NAN);
    }
    let cp = (usl - lsl) / (6.0 * sigma);
    let cpu = (usl - mean) / (3.0 * sigma);
    let cpl = (mean - lsl) / (3.0 * sigma);
    let cpk = cpu.min(cpl);
    (cp, cpk)
}

/// Peak bin center DN and fraction of samples in that bin.
fn histogram_peak(samples: &VecDeque<f64>, bins: usize) -> (f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut counts = vec![0u32; bins];
    for &v in samples {
        let idx = ((v / 256.0) * bins as f64).floor() as usize;
        let idx = idx.min(bins - 1);
        counts[idx] += 1;
    }
    let peak_idx = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, &c)| c)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let peak_count = counts[peak_idx];
    let peak_dn = (peak_idx as f64 + 0.5) * (256.0 / bins as f64);
    let ratio = peak_count as f64 / samples.len() as f64;
    (peak_dn, ratio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sfi_plugin_host::{BBox, Detection, Metric, TaskResponse};

    #[test]
    fn rolling_ng_rate_updates() {
        let engine = SpcEngine::new(4);
        let ok = TaskResponse {
            task_id: 1,
            status: "ok".into(),
            message: String::new(),
            detections: vec![],
            metrics: vec![Metric {
                name: "gray_mean".into(),
                value: 100.0,
                unit: "dn".into(),
            }],
        };
        let ng = TaskResponse {
            task_id: 2,
            status: "ok".into(),
            message: String::new(),
            detections: vec![Detection {
                class_id: 1,
                label: "d".into(),
                score: 0.9,
                bbox: BBox {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
            }],
            metrics: vec![Metric {
                name: "gray_mean".into(),
                value: 120.0,
                unit: "dn".into(),
            }],
        };

        engine.ingest(1, &ok, 1);
        let s2 = engine.ingest(2, &ng, 2);
        let ng_rate = s2
            .values
            .iter()
            .find(|v| v.name == "ng_rate")
            .unwrap()
            .value;
        assert!((ng_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn cp_cpk_from_window() {
        let limits = SpcLimitsSection {
            metric: "gray_mean".into(),
            usl: 130.0,
            lsl: 110.0,
            target: 120.0,
        };
        let engine = SpcEngine::with_config(8, limits, 16);
        for i in 0..8 {
            let resp = TaskResponse {
                task_id: i,
                status: "ok".into(),
                message: String::new(),
                detections: vec![],
                metrics: vec![Metric {
                    name: "gray_mean".into(),
                    value: 118.0 + (i as f64) * 0.5,
                    unit: "dn".into(),
                }],
            };
            engine.ingest(i, &resp, i);
        }
        let snap = engine.last().unwrap();
        let cp = snap.values.iter().find(|v| v.name == "cp").unwrap().value;
        let cpk = snap.values.iter().find(|v| v.name == "cpk").unwrap().value;
        assert!(cp > 1.0);
        assert!(cpk > 0.5);
        let std = snap
            .values
            .iter()
            .find(|v| v.name == "gray_mean_std")
            .unwrap()
            .value;
        assert!(std > 0.0);
    }
}

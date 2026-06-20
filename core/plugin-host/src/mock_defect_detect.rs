//! Mock defect-detect sidecar — mirrors Julia SFIDefectDetect for CI.

use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::mock_vision::encode_framed_response;
use crate::plugin_wire::{BBox, Detection, Metric, TaskRequest, TaskResponse};
use crate::shm_gray8::{
    bright_pixel_count, bright_pixels_in_roi_limit, crop_roi, edge_caliper_horizontal,
    edge_caliper_vertical, extract_template, gray_mean, gray_mean_roi,
    measure_circle_diameter_horizontal, measure_line_width_horizontal, mmap_gray8, ncc_match,
    ncc_score_at, read_gray8,
};

pub async fn run_mock_defect_detect_sidecar(socket_path: &Path) -> std::io::Result<()> {
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }
    if let Some(parent) = socket_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let listener = UnixListener::bind(socket_path)?;
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream).await {
                tracing::warn!(error = %err, "mock defect-detect connection error");
            }
        });
    }
}

async fn handle_connection(mut stream: UnixStream) -> std::io::Result<()> {
    loop {
        let len = match stream.read_u32_le().await {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        };
        let mut body = vec![0u8; len as usize];
        stream.read_exact(&mut body).await?;
        let req: TaskRequest = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(error = %err, "invalid task request");
                continue;
            }
        };
        let resp = mock_defect_response(&req);
        let framed = encode_framed_response(&resp)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        stream.write_all(&framed).await?;
    }
}

pub fn mock_defect_response(req: &TaskRequest) -> TaskResponse {
    if req.task_type.starts_with("vision.inspect.") {
        return mock_inspect_response(req);
    }
    if req.task_type.starts_with("vision.measure.") {
        return mock_measure_response(req);
    }

    let threshold = req
        .params
        .get("threshold")
        .and_then(|v| v.as_u64())
        .unwrap_or(128) as u8;

    if let Ok(mmap) = mmap_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset) {
        return response_from_mmap(req, &mmap, threshold);
    }
    if let Ok(pixels) = read_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset) {
        return response_from_pixels(req, &pixels, threshold);
    }

    fallback_response(req, threshold)
}

fn measure_cfg(req: &TaskRequest) -> (f64, u32, u32, u32, u32, String, String, f64, f64) {
    let measure = req.params.get("measure");
    let edge = measure.and_then(|m| m.get("edge"));
    let dim = measure.and_then(|m| m.get("dimension"));
    let mm = measure
        .and_then(|m| m.get("mmPerPixel"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let x0 = edge
        .and_then(|e| e.get("x0"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let y0 = edge
        .and_then(|e| e.get("y0"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let x1 = edge
        .and_then(|e| e.get("x1"))
        .and_then(|v| v.as_u64())
        .unwrap_or(req.frame.width as u64) as u32;
    let y1 = edge
        .and_then(|e| e.get("y1"))
        .and_then(|v| v.as_u64())
        .unwrap_or(y0 as u64) as u32;
    let polarity = edge
        .and_then(|e| e.get("polarity"))
        .and_then(|v| v.as_str())
        .unwrap_or("rising")
        .to_string();
    let dim_kind = dim
        .and_then(|d| d.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("edge_position")
        .to_string();
    let nominal = dim
        .and_then(|d| d.get("nominal"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let tolerance = dim
        .and_then(|d| d.get("tolerance"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    (mm, x0, y0, x1, y1, polarity, dim_kind, nominal, tolerance)
}

pub fn mock_measure_response(req: &TaskRequest) -> TaskResponse {
    if let Ok(mmap) = mmap_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset) {
        return measure_response_from_pixels(req, &mmap, "mock measure (shm-mmap)");
    }
    if let Ok(pixels) = read_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset) {
        return measure_response_from_pixels(req, &pixels, "mock measure (shm)");
    }
    TaskResponse {
        task_id: req.task_id,
        status: "error".into(),
        message: "mock measure: no shm".into(),
        detections: vec![],
        metrics: vec![],
    }
}

fn measure_response_from_pixels(req: &TaskRequest, pixels: &[u8], msg: &str) -> TaskResponse {
    let (work, w, h) = apply_roi(req, pixels);
    let (mm, x0, y0, x1, y1, polarity, dim_kind, nominal, _tolerance) = measure_cfg(req);
    let x1_eff = if x1 > 0 { x1.min(w) } else { w };
    let y1_eff = if y1 > 0 { y1.min(h) } else { y0 };

    if req.task_type == "vision.measure.dimension" && dim_kind == "line_width" {
        let result = measure_line_width_horizontal(&work, w, h, y0, x0, x1_eff.saturating_sub(1));
        if let Some((width_px, left_x, right_x, strength)) = result {
            let mut metrics = vec![
                Metric {
                    name: "line_width_px".into(),
                    value: width_px,
                    unit: "px".into(),
                },
                Metric {
                    name: "edge_strength".into(),
                    value: strength,
                    unit: "dn".into(),
                },
            ];
            if mm > 0.0 {
                metrics.push(Metric {
                    name: "line_width_mm".into(),
                    value: width_px * mm,
                    unit: "mm".into(),
                });
            }
            if nominal > 0.0 {
                metrics.push(Metric {
                    name: "dimension_deviation_px".into(),
                    value: width_px - nominal,
                    unit: "px".into(),
                });
                if mm > 0.0 {
                    metrics.push(Metric {
                        name: "dimension_deviation_mm".into(),
                        value: (width_px - nominal) * mm,
                        unit: "mm".into(),
                    });
                }
            }
            return TaskResponse {
                task_id: req.task_id,
                status: "ok".into(),
                message: msg.into(),
                detections: vec![Detection {
                    class_id: 11,
                    label: "line_width".into(),
                    score: 0.9,
                    bbox: BBox {
                        x: left_x as f32,
                        y: y0 as f32,
                        width: (right_x - left_x) as f32,
                        height: 1.0,
                    },
                }],
                metrics,
            };
        }
        return measure_error(req, msg, "no edges for line_width");
    }

    if req.task_type == "vision.measure.dimension" && dim_kind == "circle_diameter" {
        let result = measure_circle_diameter_horizontal(&work, w, h, y0, x0, x1_eff.saturating_sub(1));
        if let Some((diam, radius, left_x, right_x)) = result {
            let mut metrics = vec![
                Metric {
                    name: "circle_diameter_px".into(),
                    value: diam,
                    unit: "px".into(),
                },
                Metric {
                    name: "circle_radius_px".into(),
                    value: radius,
                    unit: "px".into(),
                },
            ];
            if mm > 0.0 {
                metrics.push(Metric {
                    name: "circle_diameter_mm".into(),
                    value: diam * mm,
                    unit: "mm".into(),
                });
            }
            if nominal > 0.0 {
                metrics.push(Metric {
                    name: "dimension_deviation_px".into(),
                    value: diam - nominal,
                    unit: "px".into(),
                });
            }
            let cx = (left_x + right_x) / 2.0;
            return TaskResponse {
                task_id: req.task_id,
                status: "ok".into(),
                message: msg.into(),
                detections: vec![Detection {
                    class_id: 11,
                    label: "circle".into(),
                    score: 0.9,
                    bbox: BBox {
                        x: (cx - radius) as f32,
                        y: (y0 as f64 - 1.0) as f32,
                        width: diam as f32,
                        height: 2.0,
                    },
                }],
                metrics,
            };
        }
        return measure_error(req, msg, "no circle diameter");
    }

    let edge = if y0 == y1_eff {
        edge_caliper_horizontal(&work, w, h, y0, x0, x1_eff.saturating_sub(1), &polarity)
    } else if x0 == x1_eff {
        edge_caliper_vertical(&work, w, h, x0, y0, y1_eff.saturating_sub(1), &polarity)
    } else {
        edge_caliper_horizontal(&work, w, h, y0, x0, x1_eff.saturating_sub(1), &polarity)
    };
    if let Some((pos, strength)) = edge {
        let mut metrics = vec![
            Metric {
                name: "edge_position_px".into(),
                value: pos,
                unit: "px".into(),
            },
            Metric {
                name: "edge_strength".into(),
                value: strength,
                unit: "dn".into(),
            },
        ];
        if mm > 0.0 {
            metrics.push(Metric {
                name: "edge_position_mm".into(),
                value: pos * mm,
                unit: "mm".into(),
            });
        }
        if nominal > 0.0 {
            metrics.push(Metric {
                name: "edge_deviation_px".into(),
                value: pos - nominal,
                unit: "px".into(),
            });
            if mm > 0.0 {
                metrics.push(Metric {
                    name: "edge_deviation_mm".into(),
                    value: (pos - nominal) * mm,
                    unit: "mm".into(),
                });
            }
        }
        return TaskResponse {
            task_id: req.task_id,
            status: "ok".into(),
            message: msg.into(),
            detections: vec![Detection {
                class_id: 10,
                label: "edge".into(),
                score: (strength / 255.0).min(0.99) as f32,
                bbox: BBox {
                    x: (pos - 1.0) as f32,
                    y: (y0 as f64 - 1.0) as f32,
                    width: 2.0,
                    height: 2.0,
                },
            }],
            metrics,
        };
    }
    measure_error(req, msg, "no edge")
}

fn inspect_cfg(req: &TaskRequest) -> (u32, u32, u32, u32, u32, u32, u32, u32, f64, f64, f64) {
    let inspect = req.params.get("inspect");
    let search = inspect.and_then(|i| i.get("search"));
    let tpl = inspect.and_then(|i| i.get("template"));
    let sx0 = search
        .and_then(|s| s.get("x0"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let sy0 = search
        .and_then(|s| s.get("y0"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let sx1 = search
        .and_then(|s| s.get("x1"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let sy1 = search
        .and_then(|s| s.get("y1"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let tx = tpl
        .and_then(|t| t.get("x"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let ty = tpl
        .and_then(|t| t.get("y"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let tw = tpl
        .and_then(|t| t.get("width"))
        .and_then(|v| v.as_u64())
        .unwrap_or(16) as u32;
    let th = tpl
        .and_then(|t| t.get("height"))
        .and_then(|v| v.as_u64())
        .unwrap_or(16) as u32;
    let min_score = inspect
        .and_then(|i| i.get("minScore"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.8);
    let expected_x = inspect
        .and_then(|i| i.get("expectedX"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let expected_y = inspect
        .and_then(|i| i.get("expectedY"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    (
        sx0, sy0, sx1, sy1, tx, ty, tw, th, min_score, expected_x, expected_y,
    )
}

pub fn mock_inspect_response(req: &TaskRequest) -> TaskResponse {
    if let Ok(mmap) = mmap_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset) {
        return inspect_response_from_pixels(req, &mmap, "mock inspect (shm-mmap)");
    }
    if let Ok(pixels) = read_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset) {
        return inspect_response_from_pixels(req, &pixels, "mock inspect (shm)");
    }
    TaskResponse {
        task_id: req.task_id,
        status: "error".into(),
        message: "mock inspect: no shm".into(),
        detections: vec![],
        metrics: vec![],
    }
}

fn inspect_response_from_pixels(req: &TaskRequest, pixels: &[u8], msg: &str) -> TaskResponse {
    let (work, w, h) = apply_roi(req, pixels);
    let (sx0, sy0, sx1, sy1, tx, ty, tw, th, min_score, expected_x, expected_y) =
        inspect_cfg(req);
    let sx1_eff = if sx1 > 0 { sx1.min(w) } else { w.saturating_sub(tw) };
    let sy1_eff = if sy1 > 0 { sy1.min(h) } else { h.saturating_sub(th) };
    let (template, tw, th) = extract_template(&work, w, h, tx, ty, tw, th);
    if template.is_empty() {
        return inspect_error(req, msg, "invalid template roi");
    }

    let (match_x, match_y, score) = if req.task_type == "vision.inspect.presence" {
        let s = ncc_score_at(&work, w, h, &template, tw, th, tx, ty);
        (tx, ty, s)
    } else {
        let m = ncc_match(&work, w, h, &template, tw, th, sx0, sy0, sx1_eff, sy1_eff)
            .unwrap_or((0, 0, -1.0));
        (m.0, m.1, m.2)
    };

    if score < -0.5 {
        return inspect_error(req, msg, "ncc match failed");
    }

    let status = if score >= min_score { "ok" } else { "error" };
    let mut metrics = vec![
        Metric {
            name: "ncc_score".into(),
            value: score,
            unit: "ratio".into(),
        },
        Metric {
            name: "template_offset_x_px".into(),
            value: match_x as f64,
            unit: "px".into(),
        },
        Metric {
            name: "template_offset_y_px".into(),
            value: match_y as f64,
            unit: "px".into(),
        },
    ];
    if expected_x > 0.0 || expected_y > 0.0 {
        metrics.push(Metric {
            name: "position_deviation_x_px".into(),
            value: match_x as f64 - expected_x,
            unit: "px".into(),
        });
        metrics.push(Metric {
            name: "position_deviation_y_px".into(),
            value: match_y as f64 - expected_y,
            unit: "px".into(),
        });
    }

    let message = if status == "ok" {
        msg.to_string()
    } else {
        format!("{}: ncc below min_score ({} < {})", msg, score, min_score)
    };

    TaskResponse {
        task_id: req.task_id,
        status: status.into(),
        message,
        detections: vec![Detection {
            class_id: 12,
            label: "template".into(),
            score: (score.min(0.99) as f32).max(0.0),
            bbox: BBox {
                x: match_x as f32,
                y: match_y as f32,
                width: tw as f32,
                height: th as f32,
            },
        }],
        metrics,
    }
}

fn inspect_error(req: &TaskRequest, msg: &str, err: &str) -> TaskResponse {
    TaskResponse {
        task_id: req.task_id,
        status: "error".into(),
        message: format!("{}: {}", msg, err),
        detections: vec![],
        metrics: vec![],
    }
}

fn measure_error(req: &TaskRequest, msg: &str, err: &str) -> TaskResponse {
    TaskResponse {
        task_id: req.task_id,
        status: "error".into(),
        message: format!("{}: {}", msg, err),
        detections: vec![],
        metrics: vec![],
    }
}

fn response_from_mmap(req: &TaskRequest, mmap: &[u8], threshold: u8) -> TaskResponse {
    let (rx, ry, rw, rh) = roi_bounds(req);
    let bright = bright_pixels_in_roi_limit(mmap, req.frame.stride, rx, ry, rw, rh, threshold, 1);
    let has_defect = bright > 0;
    let gmean = gray_mean_roi(mmap, req.frame.stride, req.frame.width, rx, ry, rw, rh);

    TaskResponse {
        task_id: req.task_id,
        status: "ok".into(),
        message: "mock defect-detect (shm-mmap)".into(),
        detections: if has_defect {
            vec![Detection {
                class_id: 1,
                label: "surface_defect".into(),
                score: 0.9,
                bbox: BBox {
                    x: rw as f32 * 0.25,
                    y: rh as f32 * 0.25,
                    width: rw as f32 * 0.5,
                    height: rh as f32 * 0.5,
                },
            }]
        } else {
            vec![]
        },
        metrics: vec![
            Metric {
                name: "gray_mean".into(),
                value: gmean,
                unit: "dn".into(),
            },
            Metric {
                name: "bright_pixels".into(),
                value: bright as f64,
                unit: "count".into(),
            },
            Metric {
                name: "defect_components".into(),
                value: if has_defect { 1.0 } else { 0.0 },
                unit: "count".into(),
            },
        ],
    }
}

fn roi_bounds(req: &TaskRequest) -> (u32, u32, u32, u32) {
    let roi = req.params.get("roi");
    let rx = roi
        .and_then(|r| r.get("x"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let ry = roi
        .and_then(|r| r.get("y"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let rw = roi
        .and_then(|r| r.get("width"))
        .and_then(|v| v.as_u64())
        .unwrap_or(req.frame.width as u64) as u32;
    let rh = roi
        .and_then(|r| r.get("height"))
        .and_then(|v| v.as_u64())
        .unwrap_or(req.frame.height as u64) as u32;
    (rx, ry, rw, rh)
}

fn response_from_pixels(req: &TaskRequest, pixels: &[u8], threshold: u8) -> TaskResponse {
    let (work, w, h) = apply_roi(req, pixels);
    let gmean = gray_mean(&work);
    let bright = bright_pixel_count(&work, threshold);
    let has_defect = bright > 0;

    TaskResponse {
        task_id: req.task_id,
        status: "ok".into(),
        message: "mock defect-detect (shm)".into(),
        detections: if has_defect {
            vec![Detection {
                class_id: 1,
                label: "surface_defect".into(),
                score: 0.9,
                bbox: BBox {
                    x: w as f32 * 0.25,
                    y: h as f32 * 0.25,
                    width: w as f32 * 0.5,
                    height: h as f32 * 0.5,
                },
            }]
        } else {
            vec![]
        },
        metrics: vec![
            Metric {
                name: "gray_mean".into(),
                value: gmean,
                unit: "dn".into(),
            },
            Metric {
                name: "bright_pixels".into(),
                value: bright as f64,
                unit: "count".into(),
            },
            Metric {
                name: "defect_components".into(),
                value: if has_defect { 1.0 } else { 0.0 },
                unit: "count".into(),
            },
        ],
    }
}

fn apply_roi(req: &TaskRequest, pixels: &[u8]) -> (Vec<u8>, u32, u32) {
    let roi = req.params.get("roi");
    let rx = roi
        .and_then(|r| r.get("x"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let ry = roi
        .and_then(|r| r.get("y"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let rw = roi
        .and_then(|r| r.get("width"))
        .and_then(|v| v.as_u64())
        .unwrap_or(req.frame.width as u64) as u32;
    let rh = roi
        .and_then(|r| r.get("height"))
        .and_then(|v| v.as_u64())
        .unwrap_or(req.frame.height as u64) as u32;
    if rx == 0 && ry == 0 && rw == req.frame.width && rh == req.frame.height {
        return (pixels.to_vec(), req.frame.width, req.frame.height);
    }
    crop_roi(
        pixels,
        req.frame.width,
        req.frame.height,
        req.frame.stride,
        rx,
        ry,
        rw,
        rh,
    )
}

fn fallback_response(req: &TaskRequest, threshold: u8) -> TaskResponse {
    TaskResponse {
        task_id: req.task_id,
        status: "ok".into(),
        message: "mock defect-detect".into(),
        detections: vec![Detection {
            class_id: 1,
            label: "surface_defect".into(),
            score: 0.88,
            bbox: BBox {
                x: req.frame.width as f32 * 0.25,
                y: req.frame.height as f32 * 0.25,
                width: req.frame.width as f32 * 0.5,
                height: req.frame.height as f32 * 0.5,
            },
        }],
        metrics: vec![
            Metric {
                name: "gray_mean".into(),
                value: 64.0 + threshold as f64 * 0.25,
                unit: "dn".into(),
            },
            Metric {
                name: "bright_pixels".into(),
                value: threshold as f64,
                unit: "count".into(),
            },
            Metric {
                name: "defect_components".into(),
                value: 1.0,
                unit: "count".into(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_wire::{FrameRef, WIRE_API_VERSION};
    use crate::shm_gray8::{resolve_shm_path, write_measure_edge_pattern, write_test_pattern};
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio::time::timeout;

    #[test]
    fn reads_shm_for_gray_mean() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("frame.raw");
        write_test_pattern(&path, 64, 48, true).unwrap();
        let req = TaskRequest {
            api_version: WIRE_API_VERSION,
            task_id: 1,
            task_type: "vision.detect.defect".into(),
            frame: FrameRef {
                frame_id: 1,
                width: 64,
                height: 48,
                stride: 64,
                format: "gray8".into(),
                shm_name: path.to_string_lossy().into(),
                byte_length: 64 * 48,
                offset: 0,
            },
            params: serde_json::json!({ "threshold": 128 }),
        };
        let resp = mock_defect_response(&req);
        assert!(
            resp.message == "mock defect-detect (shm)"
                || resp.message == "mock defect-detect (shm-mmap)"
        );
        assert!(!resp.detections.is_empty());
        let _ = resolve_shm_path;
    }

    #[test]
    fn mock_measure_edge_from_shm() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("edge.raw");
        write_measure_edge_pattern(&path, 128, 64, 32, 48, 30, 220).unwrap();
        let req = TaskRequest {
            api_version: WIRE_API_VERSION,
            task_id: 2,
            task_type: "vision.measure.edge".into(),
            frame: FrameRef {
                frame_id: 2,
                width: 128,
                height: 64,
                stride: 128,
                format: "gray8".into(),
                shm_name: path.to_string_lossy().into(),
                byte_length: 128 * 64,
                offset: 0,
            },
            params: serde_json::json!({
                "measure": {
                    "mmPerPixel": 0.1,
                    "edge": { "x0": 0, "y0": 32, "x1": 127, "y1": 32, "polarity": "rising" }
                }
            }),
        };
        let resp = mock_measure_response(&req);
        assert_eq!(resp.status, "ok");
        let pos = resp
            .metrics
            .iter()
            .find(|m| m.name == "edge_position_px")
            .unwrap()
            .value;
        assert!(pos > 45.0 && pos < 52.0);
    }

    #[tokio::test]
    async fn mock_defect_sidecar_roundtrip() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("defect.sock");
        let sock_clone = sock.clone();
        let server = tokio::spawn(async move {
            let _ = run_mock_defect_detect_sidecar(&sock_clone).await;
        });

        for _ in 0..50 {
            if sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let req = TaskRequest {
            api_version: WIRE_API_VERSION,
            task_id: 3,
            task_type: "vision.detect.defect".into(),
            frame: FrameRef {
                frame_id: 3,
                width: 64,
                height: 48,
                stride: 64,
                format: "gray8".into(),
                shm_name: "/sfi.aoi.0".into(),
                byte_length: 64 * 48,
                offset: 0,
            },
            params: serde_json::json!({ "threshold": 128 }),
        };

        let resp = timeout(
            Duration::from_secs(2),
            crate::out_process::send_request(&sock, &req),
        )
        .await
        .expect("timeout")
        .expect("send");

        assert_eq!(resp.message, "mock defect-detect");
        assert!(resp.metrics.iter().any(|m| m.name == "gray_mean"));

        server.abort();
    }
}

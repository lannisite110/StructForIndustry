//! Mock defect-detect sidecar — mirrors Julia SFIDefectDetect for CI.

use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::mock_vision::encode_framed_response;
use crate::plugin_wire::{BBox, Detection, Metric, TaskRequest, TaskResponse};
use crate::shm_gray8::{bright_pixel_count, gray_mean, read_gray8};

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
        let framed = encode_framed_response(&resp).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        stream.write_all(&framed).await?;
    }
}

pub fn mock_defect_response(req: &TaskRequest) -> TaskResponse {
    let threshold = req
        .params
        .get("threshold")
        .and_then(|v| v.as_u64())
        .unwrap_or(128) as u8;

    if let Ok(pixels) =
        read_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset)
    {
        return response_from_pixels(req, &pixels, threshold);
    }

    fallback_response(req, threshold)
}

fn response_from_pixels(req: &TaskRequest, pixels: &[u8], threshold: u8) -> TaskResponse {
    let gmean = gray_mean(pixels);
    let bright = bright_pixel_count(pixels, threshold);
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
                    x: req.frame.width as f32 * 0.25,
                    y: req.frame.height as f32 * 0.25,
                    width: req.frame.width as f32 * 0.5,
                    height: req.frame.height as f32 * 0.5,
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
    use crate::shm_gray8::{resolve_shm_path, write_test_pattern};
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
        assert_eq!(resp.message, "mock defect-detect (shm)");
        assert!(!resp.detections.is_empty());
        let _ = resolve_shm_path;
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

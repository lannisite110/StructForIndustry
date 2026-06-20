//! Mock ai-infer sidecar for CI — mirrors `plugins/ai-infer`.

use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::mock_vision::encode_framed_response;
use crate::plugin_wire::{BBox, Detection, Metric, TaskRequest, TaskResponse};
use crate::shm_gray8::{gray_mean, read_gray8};

pub async fn run_mock_ai_infer_sidecar(socket_path: &Path) -> std::io::Result<()> {
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
                tracing::warn!(error = %err, "mock ai-infer connection error");
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
            Err(_) => continue,
        };
        if !req.task_type.starts_with("infer.") {
            continue;
        }
        let resp = mock_infer_response(&req);
        let framed = encode_framed_response(&resp)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        stream.write_all(&framed).await?;
    }
}

pub fn mock_infer_response(req: &TaskRequest) -> TaskResponse {
    let mut gmean = 128.0;
    if let Ok(pixels) = read_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset) {
        gmean = gray_mean(&pixels);
    }
    let model_path = std::env::var("SFI_ONNX_MODEL").ok();
    let uses_onnx = model_path
        .as_ref()
        .map(|p| std::path::Path::new(p).exists())
        .unwrap_or(false);

    TaskResponse {
        task_id: req.task_id,
        status: "ok".into(),
        message: if uses_onnx {
            "mock ai-infer (onnx path set)".into()
        } else {
            "mock ai-infer (onnx)".into()
        },
        detections: vec![Detection {
            class_id: 99,
            label: "ai_defect".into(),
            score: 0.95,
            bbox: BBox {
                x: req.frame.width as f32 * 0.1,
                y: req.frame.height as f32 * 0.1,
                width: req.frame.width as f32 * 0.2,
                height: req.frame.height as f32 * 0.2,
            },
        }],
        metrics: vec![
            Metric {
                name: "infer_ms".into(),
                value: 3.5,
                unit: "ms".into(),
            },
            Metric {
                name: "gray_mean".into(),
                value: gmean,
                unit: "dn".into(),
            },
            Metric {
                name: "model".into(),
                value: if uses_onnx { 1.0 } else { 0.0 },
                unit: if uses_onnx {
                    model_path.unwrap_or_default()
                } else {
                    "mock-onnx-v1".into()
                },
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_wire::{FrameRef, WIRE_API_VERSION};

    #[test]
    fn infer_response_has_detection() {
        let req = TaskRequest {
            api_version: WIRE_API_VERSION,
            task_id: 1,
            task_type: "infer.onnx".into(),
            frame: FrameRef {
                frame_id: 1,
                width: 8,
                height: 8,
                stride: 8,
                format: "gray8".into(),
                shm_name: "/missing".into(),
                byte_length: 64,
                offset: 0,
            },
            params: serde_json::json!({}),
        };
        let resp = mock_infer_response(&req);
        assert!(!resp.detections.is_empty());
    }
}

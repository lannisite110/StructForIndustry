//! Mock ONNX inference sidecar — Phase 4 scaffold.
//!
//! Accepts plugin wire v1 tasks with `task_type` prefix `infer.` and returns
//! synthetic detections + latency metrics.

use std::path::PathBuf;

use sfi_plugin_host::{
    encode_framed_response,
    shm_gray8,
    BBox, Detection, FrameRef, Metric, TaskRequest, TaskResponse, WIRE_API_VERSION,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

fn infer_response(req: &TaskRequest) -> TaskResponse {
    let mut gray_mean = 128.0;
    if let Ok(pixels) =
        shm_gray8::read_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset)
    {
        gray_mean = shm_gray8::gray_mean(&pixels);
    }

    let model_path = std::env::var("SFI_ONNX_MODEL").ok();
    let uses_onnx = model_path
        .as_ref()
        .map(|p| std::path::Path::new(p).exists())
        .unwrap_or(false);
    let message = if uses_onnx {
        "mock ai-infer (onnx path set)"
    } else {
        "mock ai-infer (onnx)"
    };

    TaskResponse {
        task_id: req.task_id,
        status: "ok".into(),
        message: message.into(),
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
                value: 4.2,
                unit: "ms".into(),
            },
            Metric {
                name: "gray_mean".into(),
                value: gray_mean,
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
        let resp = infer_response(&req);
        let framed = encode_framed_response(&resp).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        stream.write_all(&framed).await?;
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let socket_path: PathBuf = std::env::var("SFI_INFER_SOCKET")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("SFI_VISION_PLUGIN_SOCKET").map(PathBuf::from))
        .unwrap_or_else(|_| {
            if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
                PathBuf::from(runtime).join("sfi-infer.sock")
            } else {
                PathBuf::from("/tmp/sfi-infer.sock")
            }
        });

    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }
    if let Some(parent) = socket_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let listener = UnixListener::bind(&socket_path)?;
    tracing::info!(path = %socket_path.display(), "ai-infer mock sidecar ready");

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream).await {
                tracing::warn!(error = %err, "ai-infer connection error");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_infer_response() {
        let req = TaskRequest {
            api_version: WIRE_API_VERSION,
            task_id: 7,
            task_type: "infer.onnx".into(),
            frame: FrameRef {
                frame_id: 7,
                width: 64,
                height: 48,
                stride: 64,
                format: "gray8".into(),
                shm_name: "/sfi.missing".into(),
                byte_length: 64 * 48,
                offset: 0,
            },
            params: serde_json::json!({}),
        };
        let resp = infer_response(&req);
        assert_eq!(resp.message, "mock ai-infer (onnx)");
        assert!(resp.metrics.iter().any(|m| m.name == "infer_ms"));
    }
}

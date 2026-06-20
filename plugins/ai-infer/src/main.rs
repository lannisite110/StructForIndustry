mod onnx;

use std::path::PathBuf;

use sfi_plugin_host::{
    encode_framed_response, mock_infer_response, shm_gray8, BBox, Detection, Metric,
    TaskRequest, TaskResponse,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

fn infer_response(req: &TaskRequest) -> TaskResponse {
    if let Some(model) = onnx::model_path_from_env() {
        if model.exists() {
            if let Ok(pixels) =
                shm_gray8::read_gray8(&req.frame.shm_name, req.frame.byte_length, req.frame.offset)
            {
                if let Some(score) =
                    onnx::onnx_defect_score(&model, &pixels, req.frame.width, req.frame.height)
                {
                    let has_defect = score > 0.5;
                    return TaskResponse {
                        task_id: req.task_id,
                        status: "ok".into(),
                        message: "ai-infer (onnx-ref)".into(),
                        detections: if has_defect {
                            vec![Detection {
                                class_id: 99,
                                label: "ai_defect".into(),
                                score,
                                bbox: BBox {
                                    x: req.frame.width as f32 * 0.1,
                                    y: req.frame.height as f32 * 0.1,
                                    width: req.frame.width as f32 * 0.2,
                                    height: req.frame.height as f32 * 0.2,
                                },
                            }]
                        } else {
                            vec![]
                        },
                        metrics: vec![
                            Metric {
                                name: "infer_ms".into(),
                                value: 4.0,
                                unit: "ms".into(),
                            },
                            Metric {
                                name: "onnx_score".into(),
                                value: score as f64,
                                unit: "prob".into(),
                            },
                        ],
                    };
                }
            }
        }
    }
    mock_infer_response(req)
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

async fn run_sidecar(socket_path: PathBuf) -> std::io::Result<()> {
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }
    if let Some(parent) = socket_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let listener = UnixListener::bind(&socket_path)?;
    tracing::info!(path = %socket_path.display(), "ai-infer sidecar ready");
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream).await {
                tracing::warn!(error = %err, "ai-infer client error");
            }
        });
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
    run_sidecar(socket_path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sfi_plugin_host::{FrameRef, WIRE_API_VERSION};

    #[test]
    fn infer_response_ok() {
        let req = TaskRequest {
            api_version: WIRE_API_VERSION,
            task_id: 2,
            task_type: "infer.onnx".into(),
            frame: FrameRef {
                frame_id: 2,
                width: 8,
                height: 8,
                stride: 8,
                format: "gray8".into(),
                shm_name: "/nope".into(),
                byte_length: 64,
                offset: 0,
            },
            params: serde_json::json!({}),
        };
        let resp = infer_response(&req);
        assert_eq!(resp.status, "ok");
    }
}

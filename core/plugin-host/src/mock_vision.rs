//! Mock vision-2d sidecar for CI — speaks plugin wire v1 over Unix socket.

use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::plugin_wire::{BBox, Detection, Metric, TaskRequest, TaskResponse};

pub async fn run_mock_vision_sidecar(socket_path: &Path) -> std::io::Result<()> {
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
                tracing::warn!(error = %err, "mock vision connection error");
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
        let resp = mock_response(&req);
        let framed = encode_framed_response(&resp)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        stream.write_all(&framed).await?;
    }
}

fn mock_response(req: &TaskRequest) -> TaskResponse {
    let threshold = req
        .params
        .get("threshold")
        .and_then(|v| v.as_u64())
        .unwrap_or(128) as f64;

    TaskResponse {
        task_id: req.task_id,
        status: "ok".into(),
        message: "mock vision-2d".into(),
        detections: vec![Detection {
            class_id: 1,
            label: "defect".into(),
            score: 0.92,
            bbox: BBox {
                x: req.frame.width as f32 * 0.25,
                y: req.frame.height as f32 * 0.25,
                width: req.frame.width as f32 * 0.5,
                height: req.frame.height as f32 * 0.5,
            },
        }],
        metrics: vec![Metric {
            name: "bright_pixels".into(),
            value: threshold,
            unit: "count".into(),
        }],
    }
}

pub fn encode_framed_response(resp: &TaskResponse) -> Result<Vec<u8>, serde_json::Error> {
    let body = serde_json::to_vec(resp)?;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::out_process::decode_framed_response;
    use crate::plugin_wire::{FrameRef, WIRE_API_VERSION};
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio::time::timeout;

    #[tokio::test]
    async fn mock_sidecar_roundtrip() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("vision.sock");
        let sock_clone = sock.clone();
        let server = tokio::spawn(async move {
            let _ = run_mock_vision_sidecar(&sock_clone).await;
        });

        for _ in 0..50 {
            if sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let req = TaskRequest {
            api_version: WIRE_API_VERSION,
            task_id: 7,
            task_type: "vision.detect.defect".into(),
            frame: FrameRef {
                frame_id: 7,
                width: 64,
                height: 48,
                stride: 64,
                format: "gray8".into(),
                shm_name: "/sfi.pool.0".into(),
                byte_length: 64 * 48,
                offset: 0,
            },
            params: serde_json::json!({ "threshold": 200 }),
        };

        let resp = timeout(
            Duration::from_secs(2),
            crate::out_process::send_request(&sock, &req),
        )
        .await
        .expect("timeout")
        .expect("send");

        assert_eq!(resp.task_id, 7);
        assert_eq!(resp.status, "ok");
        assert!(!resp.detections.is_empty());

        let framed = encode_framed_response(&resp).unwrap();
        let back = decode_framed_response(&framed).unwrap();
        assert_eq!(back.task_id, 7);

        server.abort();
    }
}

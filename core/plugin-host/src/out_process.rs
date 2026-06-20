use std::path::Path;

use capnp::message::Builder;
use sfi_contracts::common_capnp::StatusCode;
use sfi_contracts::result_capnp::{self, ResultStatus};
use thiserror::Error;

use crate::plugin_wire::{Detection, TaskRequest, TaskResponse, WIRE_API_VERSION};

#[derive(Debug, Error)]
pub enum WireError {
    #[error("unsupported wire api_version {0}")]
    UnsupportedVersion(u32),
    #[error("plugin status error: {0}")]
    PluginError(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("capnp: {0}")]
    Capnp(#[from] capnp::Error),
}

pub fn task_request_from_hal(
    task_id: u64,
    task_type: &str,
    frame_id: u64,
    width: u32,
    height: u32,
    stride: u32,
    shm_name: &str,
    byte_length: u64,
    params: serde_json::Value,
) -> TaskRequest {
    TaskRequest {
        api_version: WIRE_API_VERSION,
        task_id,
        task_type: task_type.to_string(),
        frame: crate::plugin_wire::FrameRef {
            frame_id,
            width,
            height,
            stride,
            format: "gray8".into(),
            shm_name: shm_name.to_string(),
            byte_length,
            offset: 0,
        },
        params,
    }
}

pub fn encode_framed_request(req: &TaskRequest) -> Result<Vec<u8>, WireError> {
    if req.api_version != WIRE_API_VERSION {
        return Err(WireError::UnsupportedVersion(req.api_version));
    }
    let body = serde_json::to_vec(req)?;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn decode_framed_response(bytes: &[u8]) -> Result<TaskResponse, WireError> {
    if bytes.len() < 4 {
        return Err(WireError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "response too short",
        )));
    }
    let len = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    if bytes.len() < 4 + len {
        return Err(WireError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "truncated response body",
        )));
    }
    decode_response(&bytes[4..4 + len])
}

pub fn decode_response(body: &[u8]) -> Result<TaskResponse, WireError> {
    Ok(serde_json::from_slice(body)?)
}

pub fn result_bytes_from_response(
    resp: &TaskResponse,
    plugin_name: &str,
    plugin_version: &str,
) -> Result<Vec<u8>, WireError> {
    if resp.status == "error" {
        return Err(WireError::PluginError(resp.message.clone()));
    }

    let mut message = Builder::new_default();
    let mut result = message.init_root::<result_capnp::result::Builder>();
    result.set_task_id(resp.task_id);
    result.set_status(if resp.status == "partial" {
        ResultStatus::Partial
    } else {
        ResultStatus::Ok
    });
    result.set_code(StatusCode::Ok);
    result.set_message(&resp.message);
    result.set_plugin_name(plugin_name);
    result.set_plugin_version(plugin_version);

    if !resp.detections.is_empty() {
        let mut payload = result.init_payload();
        let mut list = payload.init_detections();
        list.set_frame_id(resp.task_id);
        list.set_source_id("vision-2d");
        let mut dets = list.init_detections(resp.detections.len() as u32);
        for (i, det) in resp.detections.iter().enumerate() {
            write_detection(dets.reborrow().get(i as u32), det);
        }
    } else if !resp.metrics.is_empty() {
        let mut payload = result.init_payload();
        let mut metrics = payload.init_metrics();
        metrics.set_frame_id(resp.task_id);
        let mut vals = metrics.init_values(resp.metrics.len() as u32);
        for (i, m) in resp.metrics.iter().enumerate() {
            let mut mv = vals.reborrow().get(i as u32);
            mv.set_name(&m.name);
            mv.set_value(m.value);
            mv.set_unit(&m.unit);
        }
    }

    let mut bytes = Vec::new();
    capnp::serialize::write_message(&mut bytes, &message)?;
    Ok(bytes)
}

/// Serialize `result.ResultEvent` for topic `task.done`.
pub fn result_event_bytes_from_response(
    resp: &TaskResponse,
    plugin_name: &str,
    plugin_version: &str,
    published_at_ns: u64,
) -> Result<Vec<u8>, WireError> {
    let result_bytes = result_bytes_from_response(resp, plugin_name, plugin_version)?;
    let result_reader = capnp::serialize::read_message(
        &result_bytes[..],
        capnp::message::ReaderOptions::new(),
    )?;
    let result_msg = result_reader.get_root::<result_capnp::result::Reader>()?;

    let mut message = Builder::new_default();
    let mut event = message.init_root::<result_capnp::result_event::Builder>();
    {
        let mut api = event.reborrow().init_api();
        api.set_major(sfi_contracts::API_VERSION_MAJOR);
        api.set_minor(sfi_contracts::API_VERSION_MINOR);
    }
    event.set_published_at_ns(published_at_ns);
    event.set_result(result_msg)?;

    let mut bytes = Vec::new();
    capnp::serialize::write_message(&mut bytes, &message)?;
    Ok(bytes)
}

fn write_detection(mut det: result_capnp::detection::Builder<'_>, src: &Detection) {
    det.set_class_id(src.class_id);
    det.set_label(&src.label);
    det.set_score(src.score);
    let mut bbox = det.init_bbox();
    bbox.set_x(src.bbox.x);
    bbox.set_y(src.bbox.y);
    bbox.set_width(src.bbox.width);
    bbox.set_height(src.bbox.height);
}

pub async fn send_request(socket_path: &Path, req: &TaskRequest) -> Result<TaskResponse, WireError> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let mut stream = UnixStream::connect(socket_path).await?;
    let framed = encode_framed_request(req)?;
    stream.write_all(&framed).await?;

    let len = stream.read_u32_le().await? as usize;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;
    decode_response(&body)
}

pub fn default_vision_socket_path() -> std::path::PathBuf {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return std::path::PathBuf::from(runtime).join("sfi-plugin-vision.sock");
    }
    std::path::PathBuf::from("/tmp/sfi-plugin-vision.sock")
}

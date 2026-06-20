//! JSON plugin wire v1 — see `core/contracts/plugin_wire.md`.

use serde::{Deserialize, Serialize};

pub const WIRE_API_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskRequest {
    pub api_version: u32,
    pub task_id: u64,
    pub task_type: String,
    pub frame: FrameRef,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameRef {
    pub frame_id: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: String,
    pub shm_name: String,
    pub byte_length: u64,
    #[serde(default)]
    pub offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskResponse {
    pub task_id: u64,
    pub status: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub detections: Vec<Detection>,
    #[serde(default)]
    pub metrics: Vec<Metric>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Detection {
    pub class_id: u32,
    pub label: String,
    pub score: f32,
    pub bbox: BBox,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    #[serde(default)]
    pub unit: String,
}

pub fn encode_request(req: &TaskRequest) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(req)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip() {
        let req = TaskRequest {
            api_version: WIRE_API_VERSION,
            task_id: 1,
            task_type: "vision.detect.defect".into(),
            frame: FrameRef {
                frame_id: 9,
                width: 4,
                height: 4,
                stride: 4,
                format: "gray8".into(),
                shm_name: "/sfi.pool.0".into(),
                byte_length: 16,
                offset: 0,
            },
            params: serde_json::json!({ "threshold": 128 }),
        };
        let bytes = encode_request(&req).unwrap();
        let back: TaskRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.task_id, 1);
    }
}

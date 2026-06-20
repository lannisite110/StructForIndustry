//! MES REST reporter — POST inspection verdict after task.done.

use serde::{Deserialize, Serialize};
use sfi_plugin_host::TaskResponse;

use crate::profile::DispatchParams;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InspectionReport {
    pub batch_id: String,
    pub frame_id: u64,
    pub verdict: String,
    pub defect_count: u32,
    pub recipe_version: String,
    pub timestamp_ns: u64,
    pub shm_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
}

impl InspectionReport {
    pub fn from_task(
        frame_id: u64,
        resp: &TaskResponse,
        params: &DispatchParams,
        timestamp_ns: u64,
        shm_name: &str,
        image_path: Option<String>,
    ) -> Self {
        let defect_count = resp
            .detections
            .iter()
            .filter(|d| is_surface_defect(d))
            .count() as u32;
        let verdict = if defect_count > 0 || resp.status == "error" {
            "NG"
        } else if params.task_type.starts_with("vision.measure.") {
            if resp.status == "error" { "NG" } else { "OK" }
        } else {
            "OK"
        };
        Self {
            batch_id: params.mes_batch_id.clone(),
            frame_id,
            verdict: verdict.into(),
            defect_count,
            recipe_version: params.recipe_version.clone(),
            timestamp_ns,
            shm_name: shm_name.to_string(),
            image_path,
        }
    }
}

fn is_surface_defect(d: &sfi_plugin_host::Detection) -> bool {
    // Measure overlays (edge / dimension) are not inspection defects.
    if d.class_id == 10 || d.class_id == 11 {
        return false;
    }
    d.class_id == 1
        || d.class_id == 99
        || d.label == "surface_defect"
        || d.label == "defect"
}

pub async fn post_mes_report(endpoint: &str, report: &InspectionReport) -> Result<(), MesError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    let resp = client.post(endpoint).json(report).send().await?;
    if !resp.status().is_success() {
        return Err(MesError::HttpStatus(resp.status().as_u16()));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MesError {
    #[error("http client: {0}")]
    Client(#[from] reqwest::Error),
    #[error("mes returned HTTP {0}")]
    HttpStatus(u16),
}

#[cfg(test)]
mod tests {
    use super::*;
    use sfi_plugin_host::{BBox, Detection, TaskResponse};

    #[test]
    fn verdict_ng_when_detections_present() {
        let resp = TaskResponse {
            task_id: 1,
            status: "ok".into(),
            message: String::new(),
            detections: vec![Detection {
                class_id: 1,
                label: "defect".into(),
                score: 0.9,
                bbox: BBox {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
            }],
            metrics: vec![],
        };
        let params = DispatchParams {
            threshold: 128,
            task_type: "vision.detect.defect".into(),
            plugin_name: "vision-2d".into(),
            recipe_version: "0.0.1".into(),
            mes_enabled: true,
            mes_endpoint: String::new(),
            mes_batch_id: "b1".into(),
            spc_window: 32,
            roi_x: 0,
            roi_y: 0,
            roi_width: 1920,
            roi_height: 1080,
        };
        let r = InspectionReport::from_task(42, &resp, &params, 100, "/sfi.test", None);
        assert_eq!(r.verdict, "NG");
        assert_eq!(r.defect_count, 1);
    }
}

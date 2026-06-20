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
}

impl InspectionReport {
    pub fn from_task(
        frame_id: u64,
        resp: &TaskResponse,
        params: &DispatchParams,
        timestamp_ns: u64,
    ) -> Self {
        let defect_count = resp.detections.len() as u32;
        let verdict = if defect_count > 0 || resp.status == "error" {
            "NG"
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
        }
    }
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
        };
        let r = InspectionReport::from_task(42, &resp, &params, 100);
        assert_eq!(r.verdict, "NG");
        assert_eq!(r.defect_count, 1);
    }
}

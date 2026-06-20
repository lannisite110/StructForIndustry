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
        let tolerance_ng = tolerance_violation(resp, params);
        let verdict = if defect_count > 0 || resp.status == "error" || tolerance_ng {
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
            shm_name: shm_name.to_string(),
            image_path,
        }
    }
}

fn is_surface_defect(d: &sfi_plugin_host::Detection) -> bool {
    if d.class_id == 10 || d.class_id == 11 || d.class_id == 12 {
        return false;
    }
    d.class_id == 1
        || d.class_id == 99
        || d.label == "surface_defect"
        || d.label == "defect"
}

fn tolerance_violation(resp: &TaskResponse, params: &DispatchParams) -> bool {
    if params.task_type.starts_with("vision.measure.") && params.measure_tolerance > 0.0 {
        if exceeds_tolerance(resp, "dimension_deviation_px", params.measure_tolerance) {
            return true;
        }
        if exceeds_tolerance(resp, "dimension_deviation_mm", params.measure_tolerance) {
            return true;
        }
        if exceeds_tolerance(resp, "edge_deviation_px", params.measure_tolerance) {
            return true;
        }
        if exceeds_tolerance(resp, "edge_deviation_mm", params.measure_tolerance) {
            return true;
        }
    }
    if params.task_type.starts_with("vision.inspect.") && params.inspect_position_tolerance > 0.0 {
        if exceeds_tolerance(resp, "position_deviation_x_px", params.inspect_position_tolerance) {
            return true;
        }
        if exceeds_tolerance(resp, "position_deviation_y_px", params.inspect_position_tolerance) {
            return true;
        }
        if exceeds_tolerance(resp, "position_deviation_x_mm", params.inspect_position_tolerance) {
            return true;
        }
        if exceeds_tolerance(resp, "position_deviation_y_mm", params.inspect_position_tolerance) {
            return true;
        }
    }
    if params.task_type.starts_with("vision.inspect.") && params.inspect_min_score > 0.0 {
        let ncc = metric_value(resp, "ncc_score");
        if let Some(s) = ncc {
            if s < params.inspect_min_score {
                return true;
            }
        }
    }
    false
}

fn exceeds_tolerance(resp: &TaskResponse, name: &str, tolerance: f64) -> bool {
    metric_value(resp, name)
        .is_some_and(|v| v.abs() > tolerance)
}

fn metric_value(resp: &TaskResponse, name: &str) -> Option<f64> {
    resp.metrics
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.value)
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
    use sfi_plugin_host::{BBox, Detection, Metric, TaskResponse};

    fn base_params(task_type: &str) -> DispatchParams {
        DispatchParams {
            threshold: 128,
            task_type: task_type.into(),
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
            measure_tolerance: 2.0,
            measure_nominal: 0.0,
            inspect_position_tolerance: 3.0,
            inspect_min_score: 0.8,
        }
    }

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
        let params = base_params("vision.detect.defect");
        let r = InspectionReport::from_task(42, &resp, &params, 100, "/sfi.test", None);
        assert_eq!(r.verdict, "NG");
        assert_eq!(r.defect_count, 1);
    }

    #[test]
    fn measure_tolerance_ng() {
        let resp = TaskResponse {
            task_id: 1,
            status: "ok".into(),
            message: String::new(),
            detections: vec![],
            metrics: vec![Metric {
                name: "edge_deviation_px".into(),
                value: 5.0,
                unit: "px".into(),
            }],
        };
        let params = base_params("vision.measure.edge");
        let r = InspectionReport::from_task(1, &resp, &params, 1, "/sfi", None);
        assert_eq!(r.verdict, "NG");
    }

    #[test]
    fn inspect_position_tolerance_ng() {
        let resp = TaskResponse {
            task_id: 1,
            status: "ok".into(),
            message: String::new(),
            detections: vec![],
            metrics: vec![Metric {
                name: "position_deviation_x_px".into(),
                value: 10.0,
                unit: "px".into(),
            }],
        };
        let params = base_params("vision.inspect.template");
        let r = InspectionReport::from_task(1, &resp, &params, 1, "/sfi", None);
        assert_eq!(r.verdict, "NG");
    }
}

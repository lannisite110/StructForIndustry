use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sfi_plugin_host::{
    result_event_bytes_from_response, send_request, task_request_from_hal, TaskResponse, WireError,
};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::bus::CoreBus;
use crate::hal_ipc::HalFrameNotify;
use crate::mes::{post_mes_report, InspectionReport};
use crate::profile::{
    algorithm_params_json, calibration_params_json, inspect_params_json, measure_params_json,
    CalibrationSection, DispatchParams, InspectSection, ProfileStore, VisionSection,
};
use crate::spc::metrics_payload_bytes;

#[derive(Clone, Debug)]
pub struct SchedulerConfig {
    pub enabled: bool,
    pub vision_socket: PathBuf,
    pub task_type: String,
    pub threshold: u64,
    pub plugin_name: String,
    pub plugin_version: String,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("SFI_SCHEDULER")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            vision_socket: std::env::var("SFI_VISION_PLUGIN_SOCKET")
                .map(PathBuf::from)
                .unwrap_or_else(|_| sfi_plugin_host::default_vision_socket_path()),
            task_type: "vision.detect.defect".into(),
            threshold: 128,
            plugin_name: "vision-2d".into(),
            plugin_version: "0.0.1".into(),
        }
    }
}

impl SchedulerConfig {
    pub fn apply_profile(&mut self, profile: &ProfileStore) {
        let p = profile.params();
        self.threshold = p.threshold;
        self.plugin_name = p.plugin_name.clone();
        if profile.params().recipe_version != self.plugin_version {
            self.plugin_version = profile.params().recipe_version.clone();
        }
        self.task_type = resolve_task_type(&p.plugin_name, &p.task_type);
        if p.plugin_name == "ai-infer" {
            self.vision_socket = std::env::var("SFI_INFER_SOCKET")
                .or_else(|_| std::env::var("SFI_VISION_PLUGIN_SOCKET"))
                .map(PathBuf::from)
                .unwrap_or_else(|_| infer_socket_default());
        }
    }
}

fn resolve_task_type(plugin: &str, profile_task: &str) -> String {
    if plugin == "ai-infer" {
        "infer.onnx".into()
    } else {
        profile_task.to_string()
    }
}

fn infer_socket_default() -> PathBuf {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime).join("sfi-infer.sock")
    } else {
        PathBuf::from("/tmp/sfi-infer.sock")
    }
}

#[derive(Debug, Default)]
pub struct SchedulerStats {
    pub tasks_dispatched: AtomicU64,
    pub tasks_completed: AtomicU64,
    pub tasks_failed: AtomicU64,
    pub last_task_id: AtomicU64,
}

impl SchedulerStats {
    pub fn snapshot(&self) -> SchedulerStatsSnapshot {
        SchedulerStatsSnapshot {
            tasks_dispatched: self.tasks_dispatched.load(Ordering::Relaxed),
            tasks_completed: self.tasks_completed.load(Ordering::Relaxed),
            tasks_failed: self.tasks_failed.load(Ordering::Relaxed),
            last_task_id: self.last_task_id.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SchedulerStatsSnapshot {
    pub tasks_dispatched: u64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub last_task_id: u64,
}

pub struct TaskScheduler {
    config: SchedulerConfig,
    stats: Arc<SchedulerStats>,
    tx: mpsc::UnboundedSender<(HalFrameNotify, CoreBus)>,
}

impl TaskScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<(HalFrameNotify, CoreBus)>();
        let config_clone = config.clone();
        let next_task_id = Arc::new(AtomicU64::new(1));
        let stats = Arc::new(SchedulerStats::default());
        let stats_worker = stats.clone();
        let ids = next_task_id.clone();

        tokio::spawn(async move {
            while let Some((notify, bus)) = rx.recv().await {
                if let Err(err) =
                    dispatch_one(&config_clone, &ids, &stats_worker, &notify, &bus).await
                {
                    warn!(error = %err, frame_id = notify.frame_id, "vision task failed");
                    stats_worker.tasks_failed.fetch_add(1, Ordering::Relaxed);
                }
            }
        });

        Self { config, stats, tx }
    }

    pub fn stats(&self) -> Arc<SchedulerStats> {
        self.stats.clone()
    }

    pub fn maybe_enqueue(&self, notify: &HalFrameNotify, bus: CoreBus) {
        if !self.config.enabled {
            return;
        }
        if let Some(profile) = bus.profile() {
            if !profile.snapshot().scheduler.auto_vision {
                return;
            }
        }
        let _ = self.tx.send((*notify, bus));
    }
}

fn resolve_params(config: &SchedulerConfig, bus: &CoreBus) -> DispatchParams {
    if let Some(profile) = bus.profile() {
        profile.params()
    } else {
        DispatchParams {
            threshold: config.threshold,
            task_type: config.task_type.clone(),
            plugin_name: config.plugin_name.clone(),
            recipe_version: config.plugin_version.clone(),
            mes_enabled: false,
            mes_endpoint: String::new(),
            mes_batch_id: "line-1".into(),
            spc_window: 32,
            roi_x: 0,
            roi_y: 0,
            roi_width: 1920,
            roi_height: 1080,
            measure_tolerance: 0.0,
            measure_nominal: 0.0,
            inspect_position_tolerance: 0.0,
            inspect_min_score: 0.8,
        }
    }
}

async fn dispatch_one(
    config: &SchedulerConfig,
    next_task_id: &AtomicU64,
    stats: &SchedulerStats,
    notify: &HalFrameNotify,
    bus: &CoreBus,
) -> Result<TaskResponse, WireError> {
    let params = resolve_params(config, bus);
    let task_id = next_task_id.fetch_add(1, Ordering::Relaxed);
    stats.tasks_dispatched.fetch_add(1, Ordering::Relaxed);
    stats.last_task_id.store(task_id, Ordering::Relaxed);

    let algorithm = bus
        .profile()
        .map(|p| algorithm_params_json(&p.snapshot().vision))
        .unwrap_or_else(|| algorithm_params_json(&VisionSection::default()));
    let measure = bus
        .profile()
        .map(|p| measure_params_json(&p.snapshot().measure))
        .unwrap_or_else(|| measure_params_json(&crate::profile::MeasureSection::default()));
    let calibration = bus
        .profile()
        .map(|p| calibration_params_json(&p.snapshot().calibration))
        .unwrap_or_else(|| calibration_params_json(&CalibrationSection::default()));
    let inspect = bus
        .profile()
        .map(|p| inspect_params_json(&p.snapshot().inspect))
        .unwrap_or_else(|| inspect_params_json(&InspectSection::default()));

    let req = task_request_from_hal(
        task_id,
        &params.task_type,
        notify.frame_id,
        notify.width,
        notify.height,
        notify.stride,
        notify.shm_name_str(),
        notify.byte_length,
        serde_json::json!({
            "threshold": params.threshold,
            "roi": {
                "x": params.roi_x,
                "y": params.roi_y,
                "width": params.roi_width,
                "height": params.roi_height,
            },
            "algorithm": algorithm,
            "measure": measure,
            "calibration": calibration,
            "inspect": inspect,
        }),
    );

    info!(
        task_id,
        frame_id = notify.frame_id,
        threshold = params.threshold,
        "dispatching vision task"
    );
    let resp = send_request(&config.vision_socket, &req).await?;
    stats.tasks_completed.fetch_add(1, Ordering::Relaxed);

    let published_at = now_ns();
    let event_bytes = result_event_bytes_from_response(
        &resp,
        &params.plugin_name,
        &config.plugin_version,
        published_at,
    )?;
    bus.publish_task_done(event_bytes, resp.task_id);

    let snapshot = bus.spc().ingest(notify.frame_id, &resp, published_at);
    if let Ok(spc_bytes) = metrics_payload_bytes(&snapshot) {
        bus.publish_spc_metrics(spc_bytes, notify.frame_id);
    }
    bus.persist_spc(&snapshot);

    let image_path = bus.frame_archive().and_then(|a| a.archive(notify));

    let report = InspectionReport::from_task(
        notify.frame_id,
        &resp,
        &params,
        published_at,
        notify.shm_name_str(),
        image_path,
    );
    bus.results().push(report.clone());

    if params.mes_enabled && !params.mes_endpoint.is_empty() {
        match post_mes_report(&params.mes_endpoint, &report).await {
            Ok(()) => {
                bus.record_mes_sent();
                info!(
                    frame_id = notify.frame_id,
                    verdict = %report.verdict,
                    "MES report sent"
                );
            }
            Err(err) => warn!(error = %err, "MES post failed"),
        }
    }

    Ok(resp)
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

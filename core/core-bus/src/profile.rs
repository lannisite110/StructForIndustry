//! Domain profile (line-realtime.yaml) — load, hot reload, runtime overrides.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LineProfile {
    pub name: String,
    pub domain: String,
    pub version: String,
    #[serde(default)]
    pub scheduler: SchedulerSection,
    #[serde(default)]
    pub vision: VisionSection,
    #[serde(default)]
    pub spc: SpcSection,
    #[serde(default)]
    pub mes: MesSection,
    #[serde(default)]
    pub compliance: ComplianceSection,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerSection {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub auto_vision: bool,
    #[serde(default = "default_true")]
    pub drop_stale_frames: bool,
    #[serde(default = "default_two")]
    pub max_queue_depth: u32,
    #[serde(default = "default_fifty_ms")]
    pub default_deadline_ms: u32,
    #[serde(default = "default_task_type")]
    pub task_type: String,
}

fn default_true() -> bool {
    true
}
fn default_two() -> u32 {
    2
}
fn default_fifty_ms() -> u32 {
    50
}
fn default_task_type() -> String {
    "vision.detect.defect".into()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct VisionSection {
    #[serde(default = "default_plugin")]
    pub plugin: String,
    #[serde(default = "default_threshold")]
    pub threshold: u64,
    #[serde(default)]
    pub roi: RoiSection,
}

fn default_plugin() -> String {
    "defect-detect".into()
}
fn default_threshold() -> u64 {
    128
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct RoiSection {
    #[serde(default)]
    pub x: u32,
    #[serde(default)]
    pub y: u32,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
}

fn default_width() -> u32 {
    1920
}
fn default_height() -> u32 {
    1080
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MesSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mes_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_batch")]
    pub batch_id: String,
}

fn default_mes_endpoint() -> String {
    "http://127.0.0.1:8090/inspection/result".into()
}
fn default_batch() -> String {
    "line-1".into()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpcSection {
    #[serde(default = "default_spc_window")]
    pub window_size: u32,
    #[serde(default)]
    pub metrics: Vec<String>,
    #[serde(default = "default_spc_persist")]
    pub persist: bool,
    #[serde(default)]
    pub persist_path: Option<String>,
    #[serde(default = "default_spc_capacity")]
    pub persist_capacity: u32,
}

fn default_spc_persist() -> bool {
    true
}
fn default_spc_capacity() -> u32 {
    4096
}

fn default_spc_window() -> u32 {
    32
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComplianceSection {
    #[serde(default)]
    pub audit_config_changes: bool,
    #[serde(default = "default_ninety")]
    pub retain_results_days: u32,
    #[serde(default)]
    pub policy: Option<String>,
    #[serde(default)]
    pub audit_log_path: Option<String>,
}

fn default_ninety() -> u32 {
    90
}

/// Runtime view used by scheduler (profile + overrides).
#[derive(Debug, Clone)]
pub struct DispatchParams {
    pub threshold: u64,
    pub task_type: String,
    pub plugin_name: String,
    pub recipe_version: String,
    pub mes_enabled: bool,
    pub mes_endpoint: String,
    pub mes_batch_id: String,
    pub spc_window: u32,
    pub roi_x: u32,
    pub roi_y: u32,
    pub roi_width: u32,
    pub roi_height: u32,
}

impl From<&LineProfile> for DispatchParams {
    fn from(p: &LineProfile) -> Self {
        Self {
            threshold: p.vision.threshold,
            task_type: p.scheduler.task_type.clone(),
            plugin_name: p.vision.plugin.clone(),
            recipe_version: p.version.clone(),
            mes_enabled: p.mes.enabled,
            mes_endpoint: p.mes.endpoint.clone(),
            mes_batch_id: p.mes.batch_id.clone(),
            spc_window: p.spc.window_size,
            roi_x: p.vision.roi.x,
            roi_y: p.vision.roi.y,
            roi_width: p.vision.roi.width,
            roi_height: p.vision.roi.height,
        }
    }
}

#[derive(Clone)]
pub struct ProfileStore {
    path: PathBuf,
    profile: Arc<RwLock<LineProfile>>,
    audit: Option<Arc<crate::audit::AuditLog>>,
}

impl ProfileStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path)?;
        let profile: LineProfile = serde_yaml::from_str(&text)?;
        Ok(Self {
            path,
            profile: Arc::new(RwLock::new(profile)),
            audit: None,
        })
    }

    pub fn load_with_audit(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let store = Self::load(path)?;
        if store.snapshot().compliance.audit_config_changes {
            let audit = Arc::new(crate::audit::AuditLog::from_compliance(
                &store.snapshot().compliance,
            )?);
            Ok(store.with_audit(audit))
        } else {
            Ok(store)
        }
    }

    pub fn with_audit(mut self, audit: Arc<crate::audit::AuditLog>) -> Self {
        self.audit = Some(audit);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn params(&self) -> DispatchParams {
        DispatchParams::from(&*self.profile.read().expect("profile lock"))
    }

    pub fn snapshot(&self) -> LineProfile {
        self.profile.read().expect("profile lock").clone()
    }

    pub fn set_threshold(&self, threshold: u64) {
        let prev = self.profile.read().expect("profile lock").vision.threshold;
        self.profile.write().expect("profile lock").vision.threshold = threshold;
        if let Some(audit) = &self.audit {
            audit.record("vision.threshold.patch", &format!("{prev} -> {threshold}"));
        }
    }

    pub fn configure_mes(&self, enabled: bool, endpoint: Option<String>, batch_id: Option<String>) {
        let mut p = self.profile.write().expect("profile lock");
        p.mes.enabled = enabled;
        if let Some(ep) = endpoint {
            p.mes.endpoint = ep;
        }
        if let Some(batch) = batch_id {
            p.mes.batch_id = batch;
        }
    }

    pub fn reload_from_disk(&self) -> Result<bool, ProfileError> {
        let text = std::fs::read_to_string(&self.path)?;
        let next: LineProfile = serde_yaml::from_str(&text)?;
        let prev = self.profile.read().expect("profile lock").clone();
        let changed = next != prev;
        if changed {
            *self.profile.write().expect("profile lock") = next.clone();
            if let Some(audit) = &self.audit {
                audit.record(
                    "profile.reload",
                    &format!(
                        "{} v{} threshold {} -> {}",
                        prev.name, prev.version, prev.vision.threshold, next.vision.threshold
                    ),
                );
            }
        }
        Ok(changed)
    }

    pub fn spawn_hot_reload(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut last_mtime = file_mtime(&self.path);
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                let mtime = file_mtime(&self.path);
                if mtime != last_mtime {
                    last_mtime = mtime;
                    match self.reload_from_disk() {
                        Ok(true) => tracing::info!(
                            path = %self.path.display(),
                            threshold = self.params().threshold,
                            "profile hot-reloaded"
                        ),
                        Ok(false) => {}
                        Err(err) => tracing::warn!(error = %err, "profile reload failed"),
                    }
                }
            }
        });
    }
}

fn file_mtime(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

pub fn default_profile_path(repo_root: &Path) -> PathBuf {
    repo_root.join("domains/industrial-inspection/profiles/line-realtime.yaml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_line_realtime_profile() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let store = ProfileStore::load(default_profile_path(&root)).expect("load");
        assert_eq!(store.snapshot().name, "line-realtime");
        assert_eq!(store.params().threshold, 128);
        assert!(store.snapshot().compliance.audit_config_changes);
    }

    #[test]
    fn loads_lab_batch_profile() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = root.join("domains/industrial-inspection/profiles/lab-batch.yaml");
        let store = ProfileStore::load(path).expect("load");
        assert_eq!(store.snapshot().name, "lab-batch");
        assert!(!store.snapshot().scheduler.drop_stale_frames);
        assert_eq!(store.snapshot().scheduler.max_queue_depth, 64);
    }

    #[test]
    fn audit_logs_threshold_change() {
        let dir = tempfile::tempdir().unwrap();
        let profile_path = dir.path().join("profile.yaml");
        let audit_path = dir.path().join("audit.log");
        std::fs::write(
            &profile_path,
            format!(
                r#"
name: audit-test
domain: industrial-inspection
version: 0.0.1
compliance:
  auditConfigChanges: true
  auditLogPath: {}
vision:
  threshold: 100
"#,
                audit_path.display()
            ),
        )
        .unwrap();
        let store = ProfileStore::load_with_audit(&profile_path).expect("load");
        store.set_threshold(120);
        let log = std::fs::read_to_string(&audit_path).unwrap();
        assert!(log.contains("vision.threshold.patch"));
        assert!(log.contains("100 -> 120"));
    }
}

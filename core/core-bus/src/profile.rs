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
        }
    }
}

#[derive(Clone)]
pub struct ProfileStore {
    path: PathBuf,
    profile: Arc<RwLock<LineProfile>>,
}

impl ProfileStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path)?;
        let profile: LineProfile = serde_yaml::from_str(&text)?;
        Ok(Self {
            path,
            profile: Arc::new(RwLock::new(profile)),
        })
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
        self.profile
            .write()
            .expect("profile lock")
            .vision
            .threshold = threshold;
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
        let changed = next != *self.profile.read().expect("profile lock");
        if changed {
            *self.profile.write().expect("profile lock") = next;
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
    }
}

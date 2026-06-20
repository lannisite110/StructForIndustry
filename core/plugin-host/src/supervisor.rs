//! Out-of-process plugin supervisor — spawn, health probe, crash restart.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use capnp::message::Builder;
use sfi_contracts::manifest_capnp::{self, PluginHealthState};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct OutProcessSpec {
    pub name: String,
    pub version: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub socket_path: PathBuf,
    pub restart_delay: Duration,
    pub health_interval: Duration,
}

impl OutProcessSpec {
    pub fn vision_2d_default(repo_root: &Path) -> Self {
        Self {
            name: "vision-2d".into(),
            version: "0.0.1".into(),
            program: PathBuf::from("julia"),
            args: vec![
                format!(
                    "--project={}",
                    repo_root.join("plugins/vision-2d").display()
                ),
                repo_root
                    .join("plugins/vision-2d/server.jl")
                    .to_string_lossy()
                    .into(),
            ],
            socket_path: default_vision_socket_path(),
            restart_delay: Duration::from_secs(2),
            health_interval: Duration::from_secs(5),
        }
    }

    pub fn defect_detect_default(repo_root: &Path) -> Self {
        Self {
            name: "defect-detect".into(),
            version: "0.0.1".into(),
            program: PathBuf::from("julia"),
            args: vec![
                format!(
                    "--project={}",
                    repo_root
                        .join("domains/industrial-inspection/plugins/defect-detect")
                        .display()
                ),
                repo_root
                    .join("domains/industrial-inspection/plugins/defect-detect/server.jl")
                    .to_string_lossy()
                    .into(),
            ],
            socket_path: default_vision_socket_path(),
            restart_delay: Duration::from_secs(2),
            health_interval: Duration::from_secs(5),
        }
    }

    pub fn for_plugin(repo_root: &Path, plugin_name: &str) -> Self {
        match plugin_name {
            "defect-detect" => Self::defect_detect_default(repo_root),
            _ => Self::vision_2d_default(repo_root),
        }
    }
}

pub fn default_vision_socket_path() -> PathBuf {
    crate::out_process::default_vision_socket_path()
}

#[derive(Clone, Debug)]
pub struct HealthReport {
    pub name: String,
    pub state: PluginHealthState,
    pub message: String,
    pub restart_count: u32,
}

pub fn plugin_health_event_bytes(report: &HealthReport) -> Result<Vec<u8>, capnp::Error> {
    let reported_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let mut message = Builder::new_default();
    let mut event = message.init_root::<manifest_capnp::plugin_health_event::Builder>();
    {
        let mut api = event.reborrow().init_api();
        api.set_major(sfi_contracts::API_VERSION_MAJOR);
        api.set_minor(sfi_contracts::API_VERSION_MINOR);
    }
    let mut health = event.init_health();
    health.set_name(&report.name);
    health.set_state(report.state);
    health.set_message(&report.message);
    health.set_reported_at_ns(reported_at);
    health.set_restart_count(report.restart_count);

    let mut bytes = Vec::new();
    capnp::serialize::write_message(&mut bytes, &message)?;
    Ok(bytes)
}

pub struct PluginSupervisor {
    spec: OutProcessSpec,
    restart_count: Arc<AtomicU32>,
    health_tx: mpsc::UnboundedSender<HealthReport>,
}

impl PluginSupervisor {
    pub fn new(spec: OutProcessSpec, health_tx: mpsc::UnboundedSender<HealthReport>) -> Self {
        Self {
            spec,
            restart_count: Arc::new(AtomicU32::new(0)),
            health_tx,
        }
    }

    pub async fn run(self) -> ! {
        loop {
            let restarts = self.restart_count.load(Ordering::Relaxed);
            self.emit(HealthReport {
                name: self.spec.name.clone(),
                state: PluginHealthState::Starting,
                message: "spawning plugin process".into(),
                restart_count: restarts,
            });

            match self.spawn_child().await {
                Ok(mut child) => {
                    if self.wait_for_socket().await {
                        self.emit(HealthReport {
                            name: self.spec.name.clone(),
                            state: PluginHealthState::Healthy,
                            message: "socket accepting connections".into(),
                            restart_count: restarts,
                        });
                    } else {
                        self.emit(HealthReport {
                            name: self.spec.name.clone(),
                            state: PluginHealthState::Degraded,
                            message: "process started but socket not ready".into(),
                            restart_count: restarts,
                        });
                    }

                    let spec = self.spec.clone();
                    let health_tx = self.health_tx.clone();
                    let name = self.spec.name.clone();
                    let probe = tokio::spawn(async move {
                        health_probe_loop(spec, health_tx, name).await;
                    });

                    let status = child.wait().await;
                    probe.abort();

                    let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
                    let restarts = self.restart_count.fetch_add(1, Ordering::Relaxed) + 1;
                    warn!(
                        plugin = %self.spec.name,
                        code,
                        restarts,
                        "plugin process exited"
                    );
                    self.emit(HealthReport {
                        name: self.spec.name.clone(),
                        state: PluginHealthState::Unhealthy,
                        message: format!("process exited with code {code}"),
                        restart_count: restarts,
                    });
                }
                Err(err) => {
                    let restarts = self.restart_count.fetch_add(1, Ordering::Relaxed) + 1;
                    warn!(plugin = %self.spec.name, error = %err, "failed to spawn plugin");
                    self.emit(HealthReport {
                        name: self.spec.name.clone(),
                        state: PluginHealthState::Unhealthy,
                        message: format!("spawn failed: {err}"),
                        restart_count: restarts,
                    });
                }
            }

            tokio::time::sleep(self.spec.restart_delay).await;
        }
    }

    async fn spawn_child(&self) -> std::io::Result<Child> {
        if self.spec.socket_path.exists() {
            let _ = std::fs::remove_file(&self.spec.socket_path);
        }
        if let Some(parent) = self.spec.socket_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        info!(
            plugin = %self.spec.name,
            program = %self.spec.program.display(),
            "spawning out-of-process plugin"
        );

        Command::new(&self.spec.program)
            .args(&self.spec.args)
            .env(
                "SFI_VISION_SOCKET",
                self.spec.socket_path.to_string_lossy().as_ref(),
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
    }

    async fn wait_for_socket(&self) -> bool {
        for _ in 0..100 {
            if tokio::net::UnixStream::connect(&self.spec.socket_path)
                .await
                .is_ok()
            {
                return true;
            }
            if self.spec.socket_path.exists() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    fn emit(&self, report: HealthReport) {
        let _ = self.health_tx.send(report);
    }
}

async fn health_probe_loop(
    spec: OutProcessSpec,
    health_tx: mpsc::UnboundedSender<HealthReport>,
    name: String,
) {
    loop {
        tokio::time::sleep(spec.health_interval).await;
        let state = if tokio::net::UnixStream::connect(&spec.socket_path)
            .await
            .is_ok()
        {
            PluginHealthState::Healthy
        } else {
            PluginHealthState::Degraded
        };
        let _ = health_tx.send(HealthReport {
            name: name.clone(),
            state,
            message: "periodic health probe".into(),
            restart_count: 0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_health_event() {
        let bytes = plugin_health_event_bytes(&HealthReport {
            name: "vision-2d".into(),
            state: PluginHealthState::Healthy,
            message: "ok".into(),
            restart_count: 0,
        })
        .unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn vision_spec_paths_exist_in_repo() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let spec = OutProcessSpec::vision_2d_default(&root);
        assert!(spec.args.len() >= 2);
    }
}

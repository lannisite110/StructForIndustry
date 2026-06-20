use std::sync::Arc;

use sfi_core_bus::{
    default_profile_path, default_store_path, run_hal_listener, run_http_server, BusConfig,
    CoreBus, ProfileStore, SpcStore, TaskScheduler,
};
use sfi_plugin_host::{plugin_health_event_bytes, OutProcessSpec, PluginSupervisor};
use tokio::sync::mpsc;
use tokio::try_join;
use tracing_subscriber::EnvFilter;

fn repo_root() -> std::path::PathBuf {
    std::env::var("SFI_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
}

fn load_profile(config: &mut BusConfig) -> Option<Arc<ProfileStore>> {
    let path = config
        .profile_path
        .clone()
        .unwrap_or_else(|| default_profile_path(&repo_root()));
    match ProfileStore::load_with_audit(&path) {
        Ok(store) => {
            let store = Arc::new(store);
            config.scheduler.apply_profile(&store);
            if store.params().mes_enabled {
                tracing::info!(endpoint = %store.params().mes_endpoint, "MES reporting enabled");
            }
            if store.snapshot().scheduler.auto_vision {
                config.scheduler.enabled = true;
            }
            tracing::info!(
                path = %path.display(),
                threshold = store.params().threshold,
                "loaded domain profile"
            );
            Some(store)
        }
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "profile not loaded");
            None
        }
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let mut config = BusConfig::default();
    if let Ok(path) = std::env::var("SFI_BUS_SOCKET") {
        config.socket_path = path.into();
    }
    if let Ok(addr) = std::env::var("SFI_BUS_HTTP") {
        config.http_addr = addr.parse().expect("SFI_BUS_HTTP must be host:port");
    }
    if let Ok(path) = std::env::var("SFI_VISION_PLUGIN_SOCKET") {
        config.scheduler.vision_socket = path.into();
    }
    if let Ok(path) = std::env::var("SFI_PROFILE") {
        config.profile_path = Some(path.into());
    }
    if std::env::var("SFI_SCHEDULER")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        config.scheduler.enabled = true;
    }

    let profile = load_profile(&mut config);
    let mut bus = CoreBus::new();
    if let Some(ref p) = profile {
        if std::env::var("SFI_MES_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            p.configure_mes(
                true,
                std::env::var("SFI_MES_ENDPOINT").ok(),
                std::env::var("SFI_MES_BATCH").ok(),
            );
        }
        bus = bus.with_profile(p.clone());
        if should_persist_spc(p) {
            let path = spc_store_path(p);
            match SpcStore::open(path, spc_store_capacity(p)) {
                Ok(store) => {
                    tracing::info!(path = %store.path().display(), "SPC trend store ready");
                    bus = bus.with_spc_store(Arc::new(store));
                }
                Err(err) => tracing::warn!(error = %err, "SPC store init failed"),
            }
        }
        p.clone().spawn_hot_reload();
    }

    let bus_for_health = bus.clone();

    if std::env::var("SFI_SUPERVISE_VISION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        let (health_tx, mut health_rx) = mpsc::unbounded_channel();
        let plugin = profile
            .as_ref()
            .map(|p| p.params().plugin_name.clone())
            .unwrap_or_else(|| "defect-detect".into());
        let mut spec = OutProcessSpec::for_plugin(&repo_root(), &plugin);
        if let Ok(path) = std::env::var("SFI_VISION_PLUGIN_SOCKET") {
            spec.socket_path = path.into();
        }
        config.scheduler.vision_socket = spec.socket_path.clone();
        tokio::spawn(PluginSupervisor::new(spec, health_tx).run());
        tokio::spawn(async move {
            while let Some(report) = health_rx.recv().await {
                if let Ok(bytes) = plugin_health_event_bytes(&report) {
                    bus_for_health.publish_plugin_health(bytes, &report.name);
                }
            }
        });
    }

    let scheduler = TaskScheduler::new(config.scheduler.clone());
    let bus = bus.with_scheduler(scheduler);
    let bus_for_http = bus.clone();
    let config_http = config.clone();

    try_join!(
        run_hal_listener(&config, bus),
        run_http_server(&config_http, &bus_for_http),
    )?;

    Ok(())
}

fn should_persist_spc(profile: &ProfileStore) -> bool {
    if std::env::var("SFI_SPC_PERSIST")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        return false;
    }
    profile.snapshot().spc.persist
        || std::env::var("SFI_SPC_PERSIST")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

fn spc_store_path(profile: &ProfileStore) -> std::path::PathBuf {
    if let Ok(path) = std::env::var("SFI_SPC_STORE") {
        return path.into();
    }
    profile
        .snapshot()
        .spc
        .persist_path
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(default_store_path)
}

fn spc_store_capacity(profile: &ProfileStore) -> usize {
    std::env::var("SFI_SPC_STORE_CAPACITY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(profile.snapshot().spc.persist_capacity as usize)
}

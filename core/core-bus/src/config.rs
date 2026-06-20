use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::bus::{BusStatsSnapshot, CoreBus};
use crate::mes::InspectionReport;
use crate::profile::{LineProfile, ProfileStore};
use crate::results::ResultStore;
use crate::scheduler::SchedulerStatsSnapshot;
use crate::spc::SpcEngine;
use crate::spc_store::SpcStore;

#[derive(Clone, Debug)]
pub struct BusConfig {
    pub socket_path: PathBuf,
    pub http_addr: SocketAddr,
    pub scheduler: crate::scheduler::SchedulerConfig,
    pub profile_path: Option<PathBuf>,
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            socket_path: crate::hal_listener::default_socket_path(),
            http_addr: "127.0.0.1:8080".parse().expect("parse addr"),
            scheduler: crate::scheduler::SchedulerConfig::default(),
            profile_path: std::env::var("SFI_PROFILE").ok().map(PathBuf::from),
        }
    }
}

#[derive(Clone)]
pub struct HttpState {
    pub bus_stats: Arc<crate::bus::BusStats>,
    pub scheduler_stats: Option<Arc<crate::scheduler::SchedulerStats>>,
    pub profile: Option<Arc<ProfileStore>>,
    pub results: ResultStore,
    pub spc: SpcEngine,
    pub spc_store: Option<Arc<SpcStore>>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    api_version_major: u16,
    api_version_minor: u16,
}

#[derive(Serialize)]
struct StatsResponse {
    frames_received: u64,
    last_frame_id: u64,
    last_timestamp_ns: u64,
    task_done_published: u64,
    plugin_health_published: u64,
    mes_reports_sent: u64,
    spc_metrics_published: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler: Option<SchedulerStatsSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<ProfileSummary>,
}

#[derive(Serialize)]
struct ProfileSummary {
    name: String,
    version: String,
    threshold: u64,
    mes_enabled: bool,
}

#[derive(Deserialize)]
struct ThresholdPatch {
    threshold: u64,
}

pub async fn run_http_server(config: &BusConfig, bus: &CoreBus) -> std::io::Result<()> {
    let state = HttpState {
        bus_stats: bus.stats(),
        scheduler_stats: bus.scheduler().map(|s| s.stats()),
        profile: bus.profile(),
        results: bus.results(),
        spc: bus.spc(),
        spc_store: bus.spc_store(),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/stats", get(stats))
        .route("/profile", get(get_profile))
        .route("/profile/vision/threshold", patch(patch_threshold))
        .route("/results/recent", get(recent_results))
        .route("/results/last", get(last_result))
        .route("/spc/metrics", get(spc_metrics))
        .route("/spc/trend", get(spc_trend))
        .route("/metrics", get(prometheus_metrics))
        .route("/", get(crate::ui::aoi_dashboard))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.http_addr).await?;
    tracing::info!(addr = %config.http_addr, "http server ready");
    axum::serve(listener, app).await
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        api_version_major: sfi_contracts::API_VERSION_MAJOR,
        api_version_minor: sfi_contracts::API_VERSION_MINOR,
    })
}

async fn stats(State(state): State<HttpState>) -> Json<StatsResponse> {
    let snap: BusStatsSnapshot = state.bus_stats.snapshot();
    Json(StatsResponse {
        frames_received: snap.frames_received,
        last_frame_id: snap.last_frame_id,
        last_timestamp_ns: snap.last_timestamp_ns,
        task_done_published: snap.task_done_published,
        plugin_health_published: snap.plugin_health_published,
        mes_reports_sent: snap.mes_reports_sent,
        spc_metrics_published: snap.spc_metrics_published,
        scheduler: state.scheduler_stats.as_ref().map(|s| s.snapshot()),
        profile: state.profile.as_ref().map(|p| {
            let params = p.params();
            ProfileSummary {
                name: p.snapshot().name,
                version: params.recipe_version,
                threshold: params.threshold,
                mes_enabled: params.mes_enabled,
            }
        }),
    })
}

async fn get_profile(State(state): State<HttpState>) -> Json<Option<LineProfile>> {
    Json(state.profile.as_ref().map(|p| p.snapshot()))
}

async fn patch_threshold(
    State(state): State<HttpState>,
    Json(body): Json<ThresholdPatch>,
) -> Result<Json<ProfileSummary>, StatusCode> {
    let profile = state.profile.as_ref().ok_or(StatusCode::NOT_FOUND)?;
    profile.set_threshold(body.threshold);
    let params = profile.params();
    tracing::info!(threshold = body.threshold, "threshold hot-updated");
    Ok(Json(ProfileSummary {
        name: profile.snapshot().name,
        version: params.recipe_version,
        threshold: params.threshold,
        mes_enabled: params.mes_enabled,
    }))
}

async fn recent_results(State(state): State<HttpState>) -> Json<Vec<InspectionReport>> {
    Json(state.results.recent(32))
}

async fn last_result(State(state): State<HttpState>) -> Json<Option<InspectionReport>> {
    Json(state.results.last())
}

async fn spc_metrics(State(state): State<HttpState>) -> Json<Option<crate::spc::SpcSnapshot>> {
    Json(state.spc.last())
}

#[derive(Deserialize)]
struct TrendQuery {
    #[serde(default = "default_trend_limit")]
    limit: usize,
}

fn default_trend_limit() -> usize {
    128
}

async fn spc_trend(
    State(state): State<HttpState>,
    axum::extract::Query(q): axum::extract::Query<TrendQuery>,
) -> Json<Vec<crate::spc::SpcSnapshot>> {
    Json(
        state
            .spc_store
            .as_ref()
            .map(|s| s.trend(q.limit))
            .unwrap_or_default(),
    )
}

async fn prometheus_metrics(State(state): State<HttpState>) -> (axum::http::StatusCode, String) {
    let snap = state.bus_stats.snapshot();
    let sched = state.scheduler_stats.as_ref().map(|s| s.snapshot());
    let body = crate::metrics::render_prometheus(&snap, sched.as_ref());
    (axum::http::StatusCode::OK, body)
}

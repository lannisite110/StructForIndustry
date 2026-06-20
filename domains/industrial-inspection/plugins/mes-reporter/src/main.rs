use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct AppState {
    last: Arc<Mutex<Option<InspectionResult>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InspectionResult {
    batch_id: String,
    frame_id: u64,
    verdict: String,
    defect_count: u32,
    recipe_version: String,
    timestamp_ns: u64,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let state = AppState::default();
    let app = Router::new()
        .route("/health", get(|| async { Json(Health { status: "ok" }) }))
        .route("/inspection/result", post(post_result))
        .route("/inspection/last", get(last_result))
        .with_state(state);

    let addr: SocketAddr = std::env::var("SFI_MES_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8090".into())
        .parse()
        .expect("SFI_MES_ADDR");
    tracing::info!(%addr, "MES reporter listening");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn post_result(
    State(state): State<AppState>,
    Json(body): Json<InspectionResult>,
) -> Json<InspectionResult> {
    tracing::info!(
        batch = %body.batch_id,
        frame_id = body.frame_id,
        verdict = %body.verdict,
        "MES ingest"
    );
    *state.last.lock().unwrap() = Some(body.clone());
    Json(body)
}

async fn last_result(State(state): State<AppState>) -> Json<Option<InspectionResult>> {
    Json(state.last.lock().unwrap().clone())
}

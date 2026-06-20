mod audit;
mod bus;
mod config;
mod frame_store;
mod hal_ipc;
mod hal_listener;
mod mes;
mod metrics;
mod profile;
mod results;
mod scheduler;
mod spc;
mod spc_store;
mod ui;

pub use audit::{default_audit_path, AuditLog};
pub use bus::{
    BusEvent, BusStats, BusStatsSnapshot, CoreBus, TOPIC_FRAME_NEW, TOPIC_PLUGIN_HEALTH,
    TOPIC_SPC_METRICS, TOPIC_TASK_DONE,
};
pub use config::{run_http_server, BusConfig, HttpState};
pub use frame_store::{frame_dir, touch_policy_marker, FrameArchive};
pub use hal_ipc::{
    HalFrameNotify, HalIpcError, NOTIFY_SIZE, POOL_ID_LEN, SHM_NAME_LEN, SOURCE_ID_LEN,
};
pub use hal_listener::{default_socket_path, run_hal_listener, HalListenerError, HalPublisher};
pub use mes::{post_mes_report, InspectionReport, MesError};
pub use metrics::render_prometheus;
pub use profile::{default_profile_path, DispatchParams, LineProfile, ProfileError, ProfileStore};
pub use results::ResultStore;
pub use scheduler::{SchedulerConfig, SchedulerStats, SchedulerStatsSnapshot, TaskScheduler};
pub use spc::{metrics_payload_bytes, SpcEngine, SpcMetricValue, SpcSnapshot};
pub use spc_store::{default_store_path, SpcStore, SpcStoreError};

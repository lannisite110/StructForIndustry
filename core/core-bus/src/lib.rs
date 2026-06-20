mod bus;
mod config;
mod hal_ipc;
mod hal_listener;
mod mes;
mod profile;
mod results;
mod scheduler;
mod spc;
mod spc_store;
mod ui;

pub use bus::{
    BusEvent, BusStats, BusStatsSnapshot, CoreBus, TOPIC_FRAME_NEW, TOPIC_PLUGIN_HEALTH,
    TOPIC_SPC_METRICS, TOPIC_TASK_DONE,
};
pub use config::{run_http_server, BusConfig, HttpState};
pub use hal_ipc::{
    HalFrameNotify, HalIpcError, NOTIFY_SIZE, POOL_ID_LEN, SHM_NAME_LEN, SOURCE_ID_LEN,
};
pub use hal_listener::{HalListenerError, HalPublisher, default_socket_path, run_hal_listener};
pub use mes::{InspectionReport, MesError, post_mes_report};
pub use profile::{default_profile_path, DispatchParams, LineProfile, ProfileError, ProfileStore};
pub use results::ResultStore;
pub use scheduler::{
    SchedulerConfig, SchedulerStats, SchedulerStatsSnapshot, TaskScheduler,
};
pub use spc::{SpcEngine, SpcMetricValue, SpcSnapshot, metrics_payload_bytes};
pub use spc_store::{default_store_path, SpcStore, SpcStoreError};

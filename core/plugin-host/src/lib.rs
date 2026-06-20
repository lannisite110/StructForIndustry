//! Plugin runtime — in-process and out-of-process (apiVersion 0).

mod in_process;
mod mock_defect_detect;
mod mock_vision;
mod out_process;
mod plugin_wire;
pub mod shm_gray8;
mod supervisor;

pub use in_process::{InProcessPlugin, LoadError, PluginInfo, ProcessError};
pub use mock_defect_detect::run_mock_defect_detect_sidecar;
pub use mock_vision::{encode_framed_response, run_mock_vision_sidecar};
pub use out_process::{
    decode_framed_response, default_vision_socket_path, encode_framed_request,
    result_bytes_from_response, result_event_bytes_from_response, send_request,
    task_request_from_hal, WireError,
};
pub use plugin_wire::{
    encode_request, BBox, Detection, FrameRef, Metric, TaskRequest, TaskResponse, WIRE_API_VERSION,
};
pub use supervisor::{plugin_health_event_bytes, HealthReport, OutProcessSpec, PluginSupervisor};

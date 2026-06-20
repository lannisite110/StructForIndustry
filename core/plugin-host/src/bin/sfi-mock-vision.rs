//! Standalone mock vision-2d sidecar for demos and CI.

use std::path::PathBuf;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let socket = std::env::var("SFI_VISION_PLUGIN_SOCKET")
        .or_else(|_| std::env::var("SFI_VISION_SOCKET"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| sfi_plugin_host::default_vision_socket_path());
    tracing::info!(path = %socket.display(), "mock vision sidecar");
    sfi_plugin_host::run_mock_vision_sidecar(&socket).await
}

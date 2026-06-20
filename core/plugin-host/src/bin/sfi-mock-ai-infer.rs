//! sfi-mock-ai-infer — mock infer.onnx sidecar for CI and demos.

use std::path::PathBuf;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let socket_path: PathBuf = std::env::var("SFI_INFER_SOCKET")
        .or_else(|_| std::env::var("SFI_VISION_PLUGIN_SOCKET"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
                PathBuf::from(runtime).join("sfi-infer.sock")
            } else {
                PathBuf::from("/tmp/sfi-infer.sock")
            }
        });
    tracing::info!(path = %socket_path.display(), "mock ai-infer sidecar");
    sfi_plugin_host::run_mock_ai_infer_sidecar(&socket_path).await
}

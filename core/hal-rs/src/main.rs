//! `sfi-capture` — synthetic HAL capture binary (Rust).
//!
//! Env:
//!   SFI_BUS_SOCKET     HAL Unix socket (default `$XDG_RUNTIME_DIR/sfi-bus.sock`)
//!   SFI_CAPTURE_FRAMES frames to publish (default 300)
//!   SFI_CAPTURE_FPS    publish rate (default 30)

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sfi_core_bus::HalPublisher;
use sfi_hal_capture::{build_notify, HEIGHT, SHM_NAME, WIDTH};
use sfi_plugin_host::shm_gray8;
use tokio::time::sleep;

fn bus_socket() -> PathBuf {
    if let Ok(p) = std::env::var("SFI_BUS_SOCKET") {
        return PathBuf::from(p);
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime).join("sfi-bus.sock");
    }
    PathBuf::from("/tmp/sfi-bus.sock")
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let bus_path = bus_socket();
    let frame_limit = env_u64("SFI_CAPTURE_FRAMES", 300);
    let fps = env_u64("SFI_CAPTURE_FPS", 30).max(1);
    let frame_interval = Duration::from_nanos(1_000_000_000 / fps);

    // Prepare the shm pool with a Gray8 test pattern (with a defect blob).
    let shm_path = shm_gray8::resolve_shm_path(SHM_NAME);
    let _ = std::fs::remove_file(&shm_path);
    shm_gray8::write_test_pattern(&shm_path, WIDTH, HEIGHT, true)?;

    for _ in 0..100 {
        if bus_path.exists() {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    let mut publisher = HalPublisher::connect(&bus_path).await?;
    tracing::info!(bus = %bus_path.display(), frames = frame_limit, "sfi-capture publishing");

    for frame_id in 0..frame_limit {
        let slot_index = (frame_id % 2) as u32;
        let notify = build_notify(frame_id, now_ns(), slot_index);
        publisher.publish(&notify).await?;
        if frame_id + 1 < frame_limit {
            sleep(frame_interval).await;
        }
    }

    let _ = std::fs::remove_file(&shm_path);
    Ok(())
}

//! Simulates PLC-triggered capture: writes Gray8 defect blob to POSIX shm, publishes HAL notify.

use std::path::PathBuf;
use std::time::Duration;

use sfi_core_bus::HalPublisher;
use sfi_line_frame::{byte_length, fill_frame, map_shm};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let bus_socket: PathBuf = std::env::var("SFI_BUS_SOCKET")
        .unwrap_or_else(|_| "/tmp/sfi-bus.sock".into())
        .into();
    let shm_name = std::env::var("SFI_LINE_SHM").unwrap_or_else(|_| "sfi.aoi.line.0".into());
    let interval_ms: u64 = std::env::var("SFI_LINE_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);
    let frame_count: u64 = std::env::var("SFI_LINE_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let byte_len = byte_length();
    let mut mmap = map_shm(&shm_name, byte_len)?;

    tracing::info!(
        bus = %bus_socket.display(),
        shm = %shm_name,
        interval_ms,
        "line publisher starting"
    );

    for _ in 0..100 {
        if bus_socket.exists() {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    let mut publisher = HalPublisher::connect(&bus_socket).await?;
    let mut frame_id: u64 = 1;
    let mut sent: u64 = 0;

    loop {
        if std::env::var("SFI_ONNX_E2E").ok().as_deref() == Some("1") {
            mmap.fill(200);
        } else {
            fill_frame(&mut mmap, frame_id, true);
        }
        let notify = sfi_line_frame::build_notify(frame_id, &shm_name, byte_len as u64);
        publisher.publish(&notify).await?;
        tracing::info!(frame_id, "published triggered frame");
        sent += 1;
        frame_id += 1;

        if frame_count > 0 && sent >= frame_count {
            break;
        }
        sleep(Duration::from_millis(interval_ms)).await;
    }

    Ok(())
}

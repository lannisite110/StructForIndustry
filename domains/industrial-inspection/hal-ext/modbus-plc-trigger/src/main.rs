mod frame_source;
mod modbus;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use frame_source::FrameSource;
use modbus::{parse_config, CoilReader};
use sfi_core_bus::HalPublisher;
use sfi_line_frame::build_notify_layout;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let config = parse_config();
    let bus_socket: PathBuf = std::env::var("SFI_BUS_SOCKET")
        .unwrap_or_else(|_| "/tmp/sfi-bus.sock".into())
        .into();
    let shm_name = std::env::var("SFI_LINE_SHM").unwrap_or_else(|_| "sfi.modbus.line.0".into());

    let mut frame_source = FrameSource::open(&shm_name)?;
    let frame_id = AtomicU64::new(1);

    tracing::info!(
        bus = %bus_socket.display(),
        modbus = %config.addr,
        coil = config.coil,
        mock = config.mock,
        poll_ms = config.poll_ms,
        v4l2 = frame_source.uses_v4l2(),
        "modbus-plc-trigger ready"
    );

    for _ in 0..100 {
        if bus_socket.exists() {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    let mut publisher = HalPublisher::connect(&bus_socket).await?;
    let mut reader = CoilReader::connect(&config).await?;

    let mut prev = false;
    let poll = Duration::from_millis(config.poll_ms.max(10));

    loop {
        let coil = reader.read_coil().await?;
        if coil && !prev {
            let id = frame_id.fetch_add(1, Ordering::Relaxed);
            let (layout, byte_len) = frame_source.fill_and_layout(id)?;
            let notify = build_notify_layout(id, &shm_name, layout, byte_len);
            publisher.publish(&notify).await?;
            tracing::info!(
                frame_id = id,
                v4l2 = frame_source.uses_v4l2(),
                "Modbus rising edge → HAL"
            );
        }
        prev = coil;
        sleep(poll).await;
    }
}

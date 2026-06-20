mod modbus;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use modbus::{parse_config, CoilReader};
use sfi_core_bus::HalPublisher;
use sfi_line_frame::{build_notify, byte_length, fill_frame, map_shm};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let config = parse_config();
    let bus_socket: PathBuf = std::env::var("SFI_BUS_SOCKET")
        .unwrap_or_else(|_| "/tmp/sfi-bus.sock".into())
        .into();
    let shm_name = std::env::var("SFI_LINE_SHM").unwrap_or_else(|_| "sfi.modbus.line.0".into());

    let byte_len = byte_length();
    let mmap = map_shm(&shm_name, byte_len)?;
    let frame_id = AtomicU64::new(1);

    tracing::info!(
        bus = %bus_socket.display(),
        modbus = %config.addr,
        coil = config.coil,
        mock = config.mock,
        poll_ms = config.poll_ms,
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
    let mut mmap = mmap;

    loop {
        let coil = reader.read_coil().await?;
        if coil && !prev {
            let id = frame_id.fetch_add(1, Ordering::Relaxed);
            fill_frame(&mut mmap, id, true);
            let notify = build_notify(id, &shm_name, byte_len as u64);
            publisher.publish(&notify).await?;
            tracing::info!(frame_id = id, "Modbus coil rising edge → HAL frame");
        }
        prev = coil;
        sleep(poll).await;
    }
}

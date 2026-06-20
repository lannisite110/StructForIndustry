//! PLC trigger gateway — accepts `TRIG` on Unix socket, publishes one HAL frame.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use sfi_core_bus::HalPublisher;
use sfi_line_frame::{byte_length, fill_frame, map_shm};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

const TRIG_MAGIC: &[u8; 4] = b"TRIG";
const ACK: &[u8; 4] = b"ACK\0";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let plc_socket: PathBuf = std::env::var("SFI_PLC_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
                PathBuf::from(runtime).join("sfi-plc.sock")
            } else {
                PathBuf::from("/tmp/sfi-plc.sock")
            }
        });
    let bus_socket: PathBuf = std::env::var("SFI_BUS_SOCKET")
        .unwrap_or_else(|_| "/tmp/sfi-bus.sock".into())
        .into();
    let shm_name = std::env::var("SFI_LINE_SHM").unwrap_or_else(|_| "sfi.aoi.line.0".into());

    if plc_socket.exists() {
        let _ = std::fs::remove_file(&plc_socket);
    }
    if let Some(parent) = plc_socket.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let byte_len = byte_length();
    let mmap = map_shm(&shm_name, byte_len)?;
    let frame_id = Arc::new(AtomicU64::new(1));

    tracing::info!(plc = %plc_socket.display(), bus = %bus_socket.display(), "plc-trigger ready");

    for _ in 0..100 {
        if bus_socket.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let publisher = Arc::new(Mutex::new(HalPublisher::connect(&bus_socket).await?));
    let state = TriggerState {
        mmap: Arc::new(Mutex::new(mmap)),
        publisher,
        shm_name,
        byte_len,
        frame_id,
    };

    let listener = UnixListener::bind(&plc_socket)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_plc_client(stream, state).await {
                tracing::warn!(error = %err, "plc client error");
            }
        });
    }
}

#[derive(Clone)]
struct TriggerState {
    mmap: Arc<Mutex<memmap2::MmapMut>>,
    publisher: Arc<Mutex<HalPublisher>>,
    shm_name: String,
    byte_len: usize,
    frame_id: Arc<AtomicU64>,
}

async fn handle_plc_client(mut stream: UnixStream, state: TriggerState) -> std::io::Result<()> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    if &buf != TRIG_MAGIC {
        return Ok(());
    }

    let id = state.frame_id.fetch_add(1, Ordering::Relaxed);
    {
        let mut mmap = state.mmap.lock().await;
        fill_frame(&mut mmap, id, true);
    }

    let notify = sfi_line_frame::build_notify(id, &state.shm_name, state.byte_len as u64);
    state.publisher.lock().await.publish(&notify).await?;
    stream.write_all(ACK).await?;

    tracing::info!(frame_id = id, "PLC trigger published frame");
    Ok(())
}

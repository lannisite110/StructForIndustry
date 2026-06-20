mod gige;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gige::{open_backend, GigEBackend, GigEConfig};
use sfi_core_bus::HalPublisher;
use sfi_line_frame::{build_notify_layout, map_shm, Gray8Layout};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio::time::sleep;

const TRIG_MAGIC: &[u8; 4] = b"TRIG";
const ACK: &[u8; 4] = b"ACK\0";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let config = load_config();
    let mode = std::env::var("SFI_GIGE_MODE").unwrap_or_else(|_| "freerun".into());

    if mode.eq_ignore_ascii_case("trigger") {
        run_trigger_mode(config).await
    } else {
        run_freerun_mode(config).await
    }
}

fn load_config() -> GigEConfig {
    GigEConfig {
        device_ip: std::env::var("SFI_GIGE_DEVICE").unwrap_or_else(|_| "192.168.1.100".into()),
        width: std::env::var("SFI_GIGE_WIDTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(640),
        height: std::env::var("SFI_GIGE_HEIGHT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(480),
        mock: std::env::var("SFI_GIGE_MOCK")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true),
    }
}

async fn run_freerun_mode(config: GigEConfig) -> std::io::Result<()> {
    let bus_socket = bus_socket_path();
    let shm_name = std::env::var("SFI_LINE_SHM").unwrap_or_else(|_| "sfi.gige.capture".into());
    let fps: u64 = std::env::var("SFI_GIGE_FPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let frame_limit: u64 = std::env::var("SFI_GIGE_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    wait_for_bus(&bus_socket).await;
    let mut publisher = HalPublisher::connect(&bus_socket).await?;

    let mut backend = open_backend(&config).map_err(std::io::Error::other)?;
    let layout = backend.layout();
    let byte_len = layout.byte_length();
    let mut mmap = map_shm(&shm_name, byte_len)?;

    tracing::info!(
        ip = %config.device_ip,
        mock = config.mock,
        width = layout.width,
        height = layout.height,
        fps,
        "gige freerun publishing"
    );

    let interval = Duration::from_nanos(1_000_000_000 / fps.max(1));
    let mut frame_id: u64 = 1;
    let mut sent: u64 = 0;

    loop {
        let frame = backend.grab(frame_id).map_err(std::io::Error::other)?;
        let n = frame.pixels.len().min(mmap.len());
        mmap[..n].copy_from_slice(&frame.pixels[..n]);

        let notify = build_notify_layout(frame_id, &shm_name, layout, byte_len as u64);
        publisher.publish(&notify).await?;
        sent += 1;
        frame_id += 1;

        if frame_limit > 0 && sent >= frame_limit {
            break;
        }
        sleep(interval).await;
    }
    Ok(())
}

async fn run_trigger_mode(config: GigEConfig) -> std::io::Result<()> {
    let plc_socket = plc_socket_path();
    let bus_socket = bus_socket_path();
    let shm_name = std::env::var("SFI_LINE_SHM").unwrap_or_else(|_| "sfi.gige.capture".into());

    if plc_socket.exists() {
        let _ = std::fs::remove_file(&plc_socket);
    }
    if let Some(parent) = plc_socket.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    wait_for_bus(&bus_socket).await;

    let backend = Arc::new(Mutex::new(
        open_backend(&config).map_err(std::io::Error::other)?,
    ));
    let layout = backend.lock().await.layout();
    let byte_len = layout.byte_length();
    let mmap = map_shm(&shm_name, byte_len)?;
    let publisher = Arc::new(Mutex::new(HalPublisher::connect(&bus_socket).await?));

    let state = TriggerState {
        backend,
        mmap: Arc::new(Mutex::new(mmap)),
        publisher,
        shm_name,
        byte_len,
        layout,
        frame_id: Arc::new(AtomicU64::new(1)),
    };

    tracing::info!(
        plc = %plc_socket.display(),
        ip = %config.device_ip,
        mock = config.mock,
        "gige trigger mode ready"
    );

    let listener = UnixListener::bind(&plc_socket)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_trig(stream, state).await {
                tracing::warn!(error = %err, "gige trigger client error");
            }
        });
    }
}

#[derive(Clone)]
struct TriggerState {
    backend: Arc<Mutex<Box<dyn GigEBackend>>>,
    mmap: Arc<Mutex<memmap2::MmapMut>>,
    publisher: Arc<Mutex<HalPublisher>>,
    shm_name: String,
    byte_len: usize,
    layout: Gray8Layout,
    frame_id: Arc<AtomicU64>,
}

async fn handle_trig(mut stream: UnixStream, state: TriggerState) -> std::io::Result<()> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    if &buf != TRIG_MAGIC {
        return Ok(());
    }

    let id = state.frame_id.fetch_add(1, Ordering::Relaxed);
    let frame = {
        let mut backend = state.backend.lock().await;
        backend.grab(id).map_err(std::io::Error::other)?
    };
    {
        let mut mmap = state.mmap.lock().await;
        let n = frame.pixels.len().min(mmap.len());
        mmap[..n].copy_from_slice(&frame.pixels[..n]);
    }

    let notify = build_notify_layout(id, &state.shm_name, state.layout, state.byte_len as u64);
    state.publisher.lock().await.publish(&notify).await?;
    stream.write_all(ACK).await?;
    tracing::info!(frame_id = id, "gige trigger published frame");
    Ok(())
}

fn bus_socket_path() -> PathBuf {
    std::env::var("SFI_BUS_SOCKET")
        .unwrap_or_else(|_| "/tmp/sfi-bus.sock".into())
        .into()
}

fn plc_socket_path() -> PathBuf {
    std::env::var("SFI_PLC_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
                PathBuf::from(runtime).join("sfi-plc.sock")
            } else {
                PathBuf::from("/tmp/sfi-plc.sock")
            }
        })
}

async fn wait_for_bus(bus_socket: &Path) {
    for _ in 0..100 {
        if bus_socket.exists() {
            return;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

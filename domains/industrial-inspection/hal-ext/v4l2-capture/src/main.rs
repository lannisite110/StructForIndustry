use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sfi_v4l2::{device_available, Camera, CaptureConfig};
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

    let device = std::env::var("SFI_V4L2_DEVICE").unwrap_or_else(|_| "/dev/video0".into());
    if !device_available(&device) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("V4L2 device missing: {device} (set SFI_V4L2_DEVICE or attach a camera)"),
        ));
    }

    let width: u32 = std::env::var("SFI_V4L2_WIDTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(640);
    let height: u32 = std::env::var("SFI_V4L2_HEIGHT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(480);
    let mode = std::env::var("SFI_V4L2_MODE").unwrap_or_else(|_| "freerun".into());

    let config = CaptureConfig {
        device: device.clone(),
        width,
        height,
    };

    if mode.eq_ignore_ascii_case("trigger") {
        run_trigger_mode(config).await
    } else {
        run_freerun_mode(config).await
    }
}

async fn run_freerun_mode(config: CaptureConfig) -> std::io::Result<()> {
    let bus_socket = bus_socket_path();
    let shm_name = std::env::var("SFI_LINE_SHM").unwrap_or_else(|_| "sfi.v4l2.capture".into());
    let fps: u64 = std::env::var("SFI_V4L2_FPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15);
    let frame_limit: u64 = std::env::var("SFI_V4L2_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let interval = Duration::from_nanos(1_000_000_000 / fps.max(1));

    wait_for_bus(&bus_socket).await;
    let mut publisher = HalPublisher::connect(&bus_socket).await?;

    let mut camera = Camera::open(&config)?;
    let layout = camera.layout();
    let byte_len = layout.byte_length();
    let mut mmap = map_shm(&shm_name, byte_len)?;

    tracing::info!(
        bus = %bus_socket.display(),
        shm = %shm_name,
        width = layout.width,
        height = layout.height,
        fps,
        "v4l2 freerun publishing"
    );

    let mut frame_id: u64 = 1;
    let mut sent: u64 = 0;
    loop {
        let frame = camera.capture_one()?;
        if frame.pixels.len() <= mmap.len() {
            mmap[..frame.pixels.len()].copy_from_slice(&frame.pixels);
        }
        let notify = build_notify_layout(frame_id, &shm_name, layout, byte_len as u64);
        publisher.publish(&notify).await?;
        tracing::debug!(frame_id, "published v4l2 frame");
        sent += 1;
        frame_id += 1;

        if frame_limit > 0 && sent >= frame_limit {
            break;
        }
        sleep(interval).await;
    }
    Ok(())
}

async fn run_trigger_mode(config: CaptureConfig) -> std::io::Result<()> {
    let plc_socket = plc_socket_path();
    let bus_socket = bus_socket_path();
    let shm_name = std::env::var("SFI_LINE_SHM").unwrap_or_else(|_| "sfi.v4l2.capture".into());

    if plc_socket.exists() {
        let _ = std::fs::remove_file(&plc_socket);
    }
    if let Some(parent) = plc_socket.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    wait_for_bus(&bus_socket).await;

    let camera = Arc::new(Mutex::new(Camera::open(&config)?));
    let layout = camera.lock().await.layout();
    let byte_len = layout.byte_length();
    let mmap = map_shm(&shm_name, byte_len)?;

    let publisher = Arc::new(Mutex::new(HalPublisher::connect(&bus_socket).await?));
    let state = TriggerState {
        camera,
        mmap: Arc::new(Mutex::new(mmap)),
        publisher,
        shm_name,
        byte_len,
        layout,
        frame_id: Arc::new(AtomicU64::new(1)),
    };

    tracing::info!(
        plc = %plc_socket.display(),
        bus = %bus_socket.display(),
        width = layout.width,
        height = layout.height,
        "v4l2 trigger mode ready (send TRIG)"
    );

    let listener = UnixListener::bind(&plc_socket)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_trig(stream, state).await {
                tracing::warn!(error = %err, "v4l2 trigger client error");
            }
        });
    }
}

#[derive(Clone)]
struct TriggerState {
    camera: Arc<Mutex<Camera>>,
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
        let mut cam = state.camera.lock().await;
        cam.capture_one()?
    };
    {
        let mut mmap = state.mmap.lock().await;
        let n = frame.pixels.len().min(mmap.len());
        mmap[..n].copy_from_slice(&frame.pixels[..n]);
    }

    let notify = build_notify_layout(id, &state.shm_name, state.layout, state.byte_len as u64);
    state.publisher.lock().await.publish(&notify).await?;
    stream.write_all(ACK).await?;
    tracing::info!(frame_id = id, "v4l2 trigger published frame");
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

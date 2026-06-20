use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{info, warn};

use crate::bus::CoreBus;
use crate::config::BusConfig;
use crate::hal_ipc::{HalFrameNotify, NOTIFY_SIZE};

pub async fn run_hal_listener(config: &BusConfig, bus: CoreBus) -> std::io::Result<()> {
    if config.socket_path.exists() {
        let _ = std::fs::remove_file(&config.socket_path);
    }
    if let Some(parent) = config.socket_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let listener = UnixListener::bind(&config.socket_path)?;
    info!(path = %config.socket_path.display(), "hal listener ready");

    loop {
        let (stream, _) = listener.accept().await?;
        let bus = bus.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_hal_connection(stream, bus).await {
                warn!(error = %err, "hal connection closed with error");
            }
        });
    }
}

async fn handle_hal_connection(mut stream: UnixStream, bus: CoreBus) -> Result<(), HalListenerError> {
    loop {
        let len = match stream.read_u32_le().await {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if len as usize != NOTIFY_SIZE {
            return Err(HalListenerError::InvalidLength(len as usize));
        }
        let mut buf = vec![0u8; NOTIFY_SIZE];
        stream.read_exact(&mut buf).await?;
        let notify = HalFrameNotify::decode(&buf)?;
        let _frame_event = bus.ingest_hal_frame(&notify);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HalListenerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    HalIpc(#[from] crate::hal_ipc::HalIpcError),
    #[error("invalid framed length {0}")]
    InvalidLength(usize),
}

pub async fn send_hal_notify(stream: &mut UnixStream, notify: &HalFrameNotify) -> std::io::Result<()> {
    let body = notify.encode();
    stream.write_u32_le(NOTIFY_SIZE as u32).await?;
    stream.write_all(&body).await?;
    Ok(())
}

pub fn default_socket_path() -> PathBuf {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return Path::new(&runtime).join("sfi-bus.sock");
    }
    PathBuf::from("/tmp/sfi-bus.sock")
}

pub struct HalPublisher {
    stream: UnixStream,
}

impl HalPublisher {
    pub async fn connect(path: &Path) -> std::io::Result<Self> {
        let stream = UnixStream::connect(path).await?;
        Ok(Self { stream })
    }

    pub async fn publish(&mut self, notify: &HalFrameNotify) -> std::io::Result<()> {
        send_hal_notify(&mut self.stream, notify).await
    }
}

pub type SharedBus = Arc<CoreBus>;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::profile::ProfileStore;
use crate::results::ResultStore;
use crate::spc::{SpcEngine, SpcSnapshot};
use crate::spc_store::SpcStore;

pub const TOPIC_FRAME_NEW: &str = "frame.new";
pub const TOPIC_TASK_DONE: &str = "task.done";
pub const TOPIC_PLUGIN_HEALTH: &str = "plugin.health";
pub const TOPIC_SPC_METRICS: &str = "spc.metrics";

#[derive(Clone, Debug)]
pub struct BusEvent {
    pub topic: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct BusStats {
    pub frames_received: AtomicU64,
    pub last_frame_id: AtomicU64,
    pub last_timestamp_ns: AtomicU64,
    pub task_done_published: AtomicU64,
    pub plugin_health_published: AtomicU64,
    pub mes_reports_sent: AtomicU64,
    pub spc_metrics_published: AtomicU64,
}

impl BusStats {
    pub fn snapshot(&self) -> BusStatsSnapshot {
        BusStatsSnapshot {
            frames_received: self.frames_received.load(Ordering::Relaxed),
            last_frame_id: self.last_frame_id.load(Ordering::Relaxed),
            last_timestamp_ns: self.last_timestamp_ns.load(Ordering::Relaxed),
            task_done_published: self.task_done_published.load(Ordering::Relaxed),
            plugin_health_published: self.plugin_health_published.load(Ordering::Relaxed),
            mes_reports_sent: self.mes_reports_sent.load(Ordering::Relaxed),
            spc_metrics_published: self.spc_metrics_published.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BusStatsSnapshot {
    pub frames_received: u64,
    pub last_frame_id: u64,
    pub last_timestamp_ns: u64,
    pub task_done_published: u64,
    pub plugin_health_published: u64,
    pub mes_reports_sent: u64,
    pub spc_metrics_published: u64,
}

struct PublishHub {
    stats: Arc<BusStats>,
    event_tx: broadcast::Sender<BusEvent>,
    results: ResultStore,
}

#[derive(Clone)]
pub struct CoreBus {
    hub: Arc<PublishHub>,
    profile: Option<Arc<ProfileStore>>,
    spc: SpcEngine,
    spc_store: Option<Arc<SpcStore>>,
    scheduler: Option<Arc<crate::scheduler::TaskScheduler>>,
}

impl CoreBus {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            hub: Arc::new(PublishHub {
                stats: Arc::new(BusStats::default()),
                event_tx,
                results: ResultStore::default(),
            }),
            profile: None,
            spc: SpcEngine::default(),
            spc_store: None,
            scheduler: None,
        }
    }

    pub fn with_spc_store(self, store: Arc<SpcStore>) -> Self {
        Self {
            spc_store: Some(store),
            ..self
        }
    }

    pub fn spc_store(&self) -> Option<Arc<SpcStore>> {
        self.spc_store.clone()
    }

    pub fn persist_spc(&self, snapshot: &SpcSnapshot) {
        if let Some(store) = &self.spc_store {
            if let Err(err) = store.append(snapshot) {
                tracing::warn!(error = %err, "spc trend persist failed");
            }
        }
    }

    pub fn with_profile(self, profile: Arc<ProfileStore>) -> Self {
        let window = profile.params().spc_window as usize;
        Self {
            spc: SpcEngine::new(window),
            profile: Some(profile),
            ..self
        }
    }

    pub fn profile(&self) -> Option<Arc<ProfileStore>> {
        self.profile.clone()
    }

    pub fn results(&self) -> ResultStore {
        self.hub.results.clone()
    }

    pub fn spc(&self) -> SpcEngine {
        self.spc.clone()
    }

    pub fn with_scheduler(self, scheduler: crate::scheduler::TaskScheduler) -> Self {
        Self {
            scheduler: Some(Arc::new(scheduler)),
            ..self
        }
    }

    pub fn scheduler(&self) -> Option<Arc<crate::scheduler::TaskScheduler>> {
        self.scheduler.clone()
    }

    pub fn stats(&self) -> Arc<BusStats> {
        self.hub.stats.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BusEvent> {
        self.hub.event_tx.subscribe()
    }

    fn emit(&self, topic: &str, bytes: Vec<u8>) {
        let event = BusEvent {
            topic: topic.to_string(),
            bytes,
        };
        let _ = self.hub.event_tx.send(event);
    }

    pub fn publish_task_done(&self, bytes: Vec<u8>, task_id: u64) {
        self.hub
            .stats
            .task_done_published
            .fetch_add(1, Ordering::Relaxed);
        tracing::info!(topic = TOPIC_TASK_DONE, task_id, "published result");
        self.emit(TOPIC_TASK_DONE, bytes);
    }

    pub fn publish_spc_metrics(&self, bytes: Vec<u8>, frame_id: u64) {
        self.hub
            .stats
            .spc_metrics_published
            .fetch_add(1, Ordering::Relaxed);
        tracing::info!(topic = TOPIC_SPC_METRICS, frame_id, "published spc metrics");
        self.emit(TOPIC_SPC_METRICS, bytes);
    }

    pub fn record_mes_sent(&self) {
        self.hub
            .stats
            .mes_reports_sent
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn publish_plugin_health(&self, bytes: Vec<u8>, plugin_name: &str) {
        self.hub
            .stats
            .plugin_health_published
            .fetch_add(1, Ordering::Relaxed);
        tracing::info!(topic = TOPIC_PLUGIN_HEALTH, plugin = plugin_name, "published health");
        self.emit(TOPIC_PLUGIN_HEALTH, bytes);
    }

    pub fn ingest_hal_frame(&self, notify: &crate::hal_ipc::HalFrameNotify) -> Vec<u8> {
        use capnp::message::Builder;
        use sfi_contracts::bus_capnp;
        use sfi_contracts::frame_capnp::PixelFormat;
        use sfi_contracts::{API_VERSION_MAJOR, API_VERSION_MINOR};

        self.hub
            .stats
            .frames_received
            .fetch_add(1, Ordering::Relaxed);
        self.hub
            .stats
            .last_frame_id
            .store(notify.frame_id, Ordering::Relaxed);
        self.hub
            .stats
            .last_timestamp_ns
            .store(notify.timestamp_ns, Ordering::Relaxed);

        let published_at = now_ns();
        let mut message = Builder::new_default();
        let mut event = message.init_root::<bus_capnp::frame_event::Builder>();
        {
            let mut api = event.reborrow().init_api();
            api.set_major(API_VERSION_MAJOR);
            api.set_minor(API_VERSION_MINOR);
        }
        event.set_published_at_ns(published_at);

        let mut frame = event.init_frame();
        frame.set_id(notify.frame_id);
        frame.set_timestamp_ns(notify.timestamp_ns);
        frame.set_source_id(notify.source_id_str());
        frame.set_width(notify.width);
        frame.set_height(notify.height);
        frame.set_stride(notify.stride);
        frame.set_format(PixelFormat::Gray8);
        frame.set_sequence(notify.sequence);

        let mut buffer = frame.init_buffer();
        {
            let mut handle = buffer.reborrow().init_handle();
            handle.set_pool_id(notify.pool_id_str());
            handle.set_slot_index(notify.slot_index);
            handle.set_byte_length(notify.byte_length);
            handle.set_offset(0);
            handle
                .reborrow()
                .init_transport()
                .set_shm_name(notify.shm_name_str());
        }
        buffer.set_generation(notify.generation);

        let mut bytes = Vec::new();
        capnp::serialize::write_message(&mut bytes, &message).expect("serialize FrameEvent");
        tracing::debug!(
            topic = TOPIC_FRAME_NEW,
            frame_id = notify.frame_id,
            source = notify.source_id_str(),
            "published frame"
        );
        self.emit(TOPIC_FRAME_NEW, bytes.clone());
        if let Some(scheduler) = &self.scheduler {
            scheduler.maybe_enqueue(notify, self.clone());
        }
        bytes
    }
}

fn now_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal_ipc::{HalFrameNotify, POOL_ID_LEN, SHM_NAME_LEN, SOURCE_ID_LEN};

    #[test]
    fn builds_frame_event_from_hal_notify() {
        let bus = CoreBus::new();
        let mut notify = HalFrameNotify {
            frame_id: 42,
            timestamp_ns: 999,
            sequence: 0,
            width: 4,
            height: 4,
            stride: 4,
            format: 1,
            source_id: [0; SOURCE_ID_LEN],
            pool_id: [0; POOL_ID_LEN],
            slot_index: 0,
            generation: 1,
            byte_length: 16,
            shm_name: [0; SHM_NAME_LEN],
        };
        notify.source_id[..11].copy_from_slice(b"synthetic-0");
        notify.pool_id[..11].copy_from_slice(b"hal.default");
        notify.shm_name[..11].copy_from_slice(b"/sfi.pool.0");

        let bytes = bus.ingest_hal_frame(&notify);
        assert!(!bytes.is_empty());
        assert_eq!(bus.stats().snapshot().frames_received, 1);

        let reader =
            capnp::serialize::read_message(&bytes[..], capnp::message::ReaderOptions::new())
                .unwrap();
        let event = reader
            .get_root::<sfi_contracts::bus_capnp::frame_event::Reader>()
            .unwrap();
        assert_eq!(event.get_frame().expect("frame").get_id(), 42);
    }
}

//! Phase 3 — 1080p shm → defect-detect latency (P95 budget 50ms mock path).

use std::time::{Duration, Instant};

use sfi_core_bus::{
    run_hal_listener, BusConfig, CoreBus, HalFrameNotify, HalPublisher, ProfileStore,
    SchedulerConfig, TaskScheduler, POOL_ID_LEN, SHM_NAME_LEN, SOURCE_ID_LEN,
};
use sfi_plugin_host::{run_mock_defect_detect_sidecar, shm_gray8};
use tempfile::tempdir;
use tokio::time::{sleep, timeout};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const BYTE_LEN: u64 = (WIDTH * HEIGHT) as u64;
const SAMPLES: usize = 20;
const P95_BUDGET_MS: u64 = 50;

#[tokio::test]
async fn bench_1080p_pipeline_under_budget() {
    let dir = tempdir().unwrap();
    let bus_socket = dir.path().join("bus.sock");
    let vision_socket = dir.path().join("vision.sock");
    let shm_tag = format!(
        "sfi-bench-1080p-{}",
        dir.path().file_name().unwrap().to_string_lossy()
    );
    let shm_path = shm_gray8::resolve_shm_path(&format!("/{shm_tag}"));
    let _ = std::fs::remove_file(&shm_path);
    shm_gray8::write_test_pattern(&shm_path, WIDTH, HEIGHT, true).expect("write shm");

    let vision_path = vision_socket.clone();
    let mock = tokio::spawn(async move {
        let _ = run_mock_defect_detect_sidecar(&vision_path).await;
    });

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile_path = root.join("domains/industrial-inspection/profiles/line-realtime.yaml");
    let profile = ProfileStore::load(&profile_path).expect("profile");

    let config = BusConfig {
        socket_path: bus_socket.clone(),
        http_addr: "127.0.0.1:0".parse().unwrap(),
        scheduler: SchedulerConfig {
            enabled: true,
            vision_socket: vision_socket.clone(),
            ..Default::default()
        },
        ..Default::default()
    };

    let scheduler = TaskScheduler::new(config.scheduler.clone());
    let bus = CoreBus::new()
        .with_profile(std::sync::Arc::new(profile))
        .with_scheduler(scheduler);

    let listener_cfg = config.clone();
    let bus_listener = bus.clone();
    let listener = tokio::spawn(async move {
        let _ = run_hal_listener(&listener_cfg, bus_listener).await;
    });

    for _ in 0..50 {
        if bus_socket.exists() && vision_socket.exists() {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    let mut publisher = HalPublisher::connect(&bus_socket).await.unwrap();

    let mut notify = HalFrameNotify {
        frame_id: 9000,
        timestamp_ns: 1,
        sequence: 0,
        width: WIDTH,
        height: HEIGHT,
        stride: WIDTH,
        format: 1,
        source_id: [0; SOURCE_ID_LEN],
        pool_id: [0; POOL_ID_LEN],
        slot_index: 0,
        generation: 1,
        byte_length: BYTE_LEN,
        shm_name: [0; SHM_NAME_LEN],
    };
    notify.source_id[..11].copy_from_slice(b"bench-1080p");
    notify.pool_id[..8].copy_from_slice(b"hal.line");
    let shm_notify = format!("/{shm_tag}");
    notify.shm_name[..shm_notify.len()].copy_from_slice(shm_notify.as_bytes());

    let mut latencies = Vec::with_capacity(SAMPLES);

    for i in 0..SAMPLES {
        notify.frame_id = 9001 + i as u64;
        notify.sequence = notify.frame_id;
        let done_before = bus.stats().snapshot().task_done_published;

        let start = Instant::now();
        publisher.publish(&notify).await.unwrap();

        timeout(Duration::from_secs(5), async {
            loop {
                if bus.stats().snapshot().task_done_published > done_before {
                    break;
                }
                sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("timeout waiting for task.done");

        latencies.push(start.elapsed());
    }

    latencies.sort();
    let p95_idx = ((SAMPLES as f64 * 0.95).ceil() as usize).saturating_sub(1);
    let p95 = latencies[p95_idx];
    let median = latencies[SAMPLES / 2];

    eprintln!(
        "1080p pipeline latency: median={:?} p95={:?} (budget {}ms)",
        median, p95, P95_BUDGET_MS
    );

    assert!(
        p95 < Duration::from_millis(P95_BUDGET_MS),
        "1080p P95 too slow: {:?} (budget {}ms)",
        p95,
        P95_BUDGET_MS
    );

    listener.abort();
    mock.abort();
    let _ = std::fs::remove_file(&shm_path);
}

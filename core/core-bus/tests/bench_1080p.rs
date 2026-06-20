//! 1080p shm → defect-detect latency smoke (mock sidecar).

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

    let mut config = BusConfig::default();
    config.socket_path = bus_socket.clone();
    config.http_addr = "127.0.0.1:0".parse().unwrap();
    config.scheduler = SchedulerConfig {
        enabled: true,
        vision_socket: vision_socket.clone(),
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
        frame_id: 9001,
        timestamp_ns: 1,
        sequence: 1,
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

    let start = Instant::now();
    publisher.publish(&notify).await.unwrap();

    timeout(Duration::from_secs(10), async {
        loop {
            if bus.stats().snapshot().task_done_published >= 1 {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("timeout waiting for task.done");

    let elapsed = start.elapsed();
    // Phase 3 target is P95 < 50ms on production HW; mock + 2MB shm read budget 500ms in CI.
    assert!(
        elapsed < Duration::from_millis(500),
        "1080p pipeline too slow: {:?}",
        elapsed
    );
    eprintln!("1080p pipeline latency: {:?}", elapsed);

    listener.abort();
    mock.abort();
    let _ = std::fs::remove_file(&shm_path);
}

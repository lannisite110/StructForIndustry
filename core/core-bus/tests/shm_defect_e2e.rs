//! E2E: POSIX shm frame → defect-detect mock (reads shm) → NG + SPC.

use std::time::Duration;

use sfi_core_bus::{
    BusConfig, CoreBus, HalFrameNotify, HalPublisher, POOL_ID_LEN, ProfileStore, SHM_NAME_LEN,
    SOURCE_ID_LEN, SchedulerConfig, SpcStore, TaskScheduler, run_hal_listener,
};
use sfi_plugin_host::{run_mock_defect_detect_sidecar, shm_gray8};
use tempfile::tempdir;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn shm_defect_detect_pipeline_produces_ng() {
    let dir = tempdir().unwrap();
    let bus_socket = dir.path().join("bus.sock");
    let vision_socket = dir.path().join("vision.sock");
    let shm_tag = format!("sfi-test-{}", dir.path().file_name().unwrap().to_string_lossy());
    let shm_path = shm_gray8::resolve_shm_path(&format!("/{shm_tag}"));
    let _ = std::fs::remove_file(&shm_path);
    shm_gray8::write_test_pattern(&shm_path, 64, 48, true).expect("write shm");

    let vision_path = vision_socket.clone();
    let mock = tokio::spawn(async move {
        let _ = run_mock_defect_detect_sidecar(&vision_path).await;
    });

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile_path = root.join("domains/industrial-inspection/profiles/line-realtime.yaml");
    let profile = ProfileStore::load(&profile_path).expect("profile");
    let spc_path = dir.path().join("spc.jsonl");
    let spc_store = std::sync::Arc::new(SpcStore::open(&spc_path, 64).expect("spc store"));

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
        .with_spc_store(spc_store.clone())
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
        frame_id: 501,
        timestamp_ns: 1,
        sequence: 1,
        width: 64,
        height: 48,
        stride: 64,
        format: 1,
        source_id: [0; SOURCE_ID_LEN],
        pool_id: [0; POOL_ID_LEN],
        slot_index: 0,
        generation: 1,
        byte_length: 64 * 48,
        shm_name: [0; SHM_NAME_LEN],
    };
    notify.source_id[..12].copy_from_slice(b"line-trigger");
    notify.pool_id[..8].copy_from_slice(b"hal.line");
    let shm_notify = format!("/{shm_tag}");
    notify.shm_name[..shm_notify.len()].copy_from_slice(shm_notify.as_bytes());

    publisher.publish(&notify).await.unwrap();

    timeout(Duration::from_secs(3), async {
        loop {
            if bus.stats().snapshot().task_done_published >= 1
                && spc_store.len() >= 1
            {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timeout");

    let last = bus.results().last().expect("result");
    assert_eq!(last.verdict, "NG");
    assert!(spc_store.trend(1)[0].frame_id == 501);

    listener.abort();
    mock.abort();
    let _ = std::fs::remove_file(&shm_path);
}

//! E2E: line-infer profile → ai-infer mock sidecar → NG + frame archive.

use std::time::Duration;

use sfi_core_bus::{
    frame_dir, run_hal_listener, BusConfig, CoreBus, FrameArchive, HalFrameNotify, HalPublisher,
    ProfileStore, SchedulerConfig, SpcStore, TaskScheduler, POOL_ID_LEN, SHM_NAME_LEN,
    SOURCE_ID_LEN,
};
use sfi_plugin_host::{run_mock_ai_infer_sidecar, shm_gray8};
use tempfile::tempdir;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn line_infer_profile_routes_to_ai_sidecar() {
    let dir = tempdir().unwrap();
    std::env::set_var("SFI_DATA_DIR", dir.path());
    let bus_socket = dir.path().join("bus.sock");
    let infer_socket = dir.path().join("infer.sock");
    let shm_tag = format!(
        "sfi-ai-e2e-{}",
        dir.path().file_name().unwrap().to_string_lossy()
    );
    let shm_path = shm_gray8::resolve_shm_path(&format!("/{shm_tag}"));
    let _ = std::fs::remove_file(&shm_path);
    shm_gray8::write_test_pattern(&shm_path, 64, 48, true).expect("write shm");

    let infer_path = infer_socket.clone();
    let mock = tokio::spawn(async move {
        let _ = run_mock_ai_infer_sidecar(&infer_path).await;
    });

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile_path = root.join("domains/industrial-inspection/profiles/line-infer.yaml");
    let profile = ProfileStore::load_with_audit(&profile_path).expect("profile");
    let spc_path = dir.path().join("spc.jsonl");

    let mut sched = SchedulerConfig {
        enabled: true,
        ..Default::default()
    };
    sched.apply_profile(&profile);
    sched.vision_socket = infer_socket.clone();

    let config = BusConfig {
        socket_path: bus_socket.clone(),
        http_addr: "127.0.0.1:0".parse().unwrap(),
        scheduler: sched.clone(),
        ..Default::default()
    };

    let archive = FrameArchive::from_compliance(&profile.snapshot().compliance).unwrap();
    let spc_store = std::sync::Arc::new(SpcStore::open(&spc_path, 64).unwrap());

    let scheduler = TaskScheduler::new(config.scheduler.clone());
    let bus = CoreBus::new()
        .with_profile(std::sync::Arc::new(profile))
        .with_spc_store(spc_store)
        .with_frame_archive(std::sync::Arc::new(archive))
        .with_scheduler(scheduler);

    let listener_cfg = config.clone();
    let bus_listener = bus.clone();
    let listener = tokio::spawn(async move {
        let _ = run_hal_listener(&listener_cfg, bus_listener).await;
    });

    for _ in 0..50 {
        if bus_socket.exists() && infer_socket.exists() {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    let mut publisher = HalPublisher::connect(&bus_socket).await.unwrap();
    let mut notify = HalFrameNotify {
        frame_id: 7001,
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
    notify.source_id[..8].copy_from_slice(b"ai-infer");
    notify.pool_id[..8].copy_from_slice(b"hal.line");
    let shm_notify = format!("/{shm_tag}");
    notify.shm_name[..shm_notify.len()].copy_from_slice(shm_notify.as_bytes());

    publisher.publish(&notify).await.unwrap();

    timeout(Duration::from_secs(3), async {
        loop {
            if bus.stats().snapshot().task_done_published >= 1 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timeout");

    let last = bus.results().last().expect("result");
    assert_eq!(last.verdict, "NG");
    assert!(last.image_path.is_some());
    assert!(frame_dir().join(last.image_path.as_ref().unwrap()).exists());

    listener.abort();
    mock.abort();
    let _ = std::fs::remove_file(&shm_path);
    std::env::remove_var("SFI_DATA_DIR");
}

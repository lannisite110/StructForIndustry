//! Phase 3 E2E: profile + vision task + MES POST.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sfi_core_bus::{
    BusConfig, CoreBus, HalFrameNotify, HalPublisher, POOL_ID_LEN, ProfileStore, SHM_NAME_LEN,
    SOURCE_ID_LEN, SchedulerConfig, TaskScheduler, TOPIC_SPC_METRICS, TOPIC_TASK_DONE,
    run_hal_listener,
};
use sfi_plugin_host::run_mock_defect_detect_sidecar;
use tempfile::tempdir;
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout};

#[derive(Clone, Default, Deserialize, Serialize, PartialEq)]
struct MesCapture {
    batch_id: String,
    frame_id: u64,
    verdict: String,
    defect_count: u32,
}

#[tokio::test]
async fn mes_receives_ng_after_defect_detection() {
    let dir = tempdir().unwrap();
    let bus_socket = dir.path().join("sfi-bus.sock");
    let vision_socket = dir.path().join("vision.sock");

    let captured = Arc::new(Mutex::new(None::<MesCapture>));
    let (tx, rx) = oneshot::channel::<u16>();

    let cap_for_server = captured.clone();
    let mock_mes = tokio::spawn(async move {
        let cap = cap_for_server.clone();
        let app = Router::new().route(
            "/inspection/result",
            post(move |Json(body): Json<MesCapture>| {
                let cap = cap.clone();
                async move {
                    *cap.lock().unwrap() = Some(body.clone());
                    Json(body)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tx.send(port).unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    let mes_port = timeout(Duration::from_secs(2), rx)
        .await
        .expect("mes port")
        .expect("port sent");
    let mes_endpoint = format!("http://127.0.0.1:{mes_port}/inspection/result");

    let vision_path = vision_socket.clone();
    let mock_vision = tokio::spawn(async move {
        let _ = run_mock_defect_detect_sidecar(&vision_path).await;
    });

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile_path = root.join("domains/industrial-inspection/profiles/line-realtime.yaml");
    let profile = Arc::new(ProfileStore::load(&profile_path).expect("profile"));
    profile.configure_mes(true, Some(mes_endpoint), None);

    let mut config = BusConfig::default();
    config.socket_path = bus_socket.clone();
    config.http_addr = "127.0.0.1:0".parse().unwrap();
    config.scheduler = SchedulerConfig {
        enabled: true,
        vision_socket: vision_socket.clone(),
        ..Default::default()
    };

    let scheduler = TaskScheduler::new(config.scheduler.clone());
    let sched_stats = scheduler.stats();
    let bus = CoreBus::new()
        .with_profile(profile.clone())
        .with_scheduler(scheduler);
    let frame_stats = bus.stats();
    let mut events = bus.subscribe();

    let listener_cfg = config.clone();
    let bus_for_listener = bus.clone();
    let listener = tokio::spawn(async move {
        let _ = run_hal_listener(&listener_cfg, bus_for_listener).await;
    });

    for _ in 0..50 {
        if bus_socket.exists() && vision_socket.exists() {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    let mut publisher = HalPublisher::connect(&bus_socket).await.unwrap();
    let mut notify = HalFrameNotify {
        frame_id: 300,
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
    notify.shm_name[..10].copy_from_slice(b"/sfi.aoi.0");

    publisher.publish(&notify).await.unwrap();

    timeout(Duration::from_secs(3), async {
        loop {
            if frame_stats.snapshot().mes_reports_sent >= 1
                && sched_stats.snapshot().tasks_completed >= 1
                && frame_stats.snapshot().spc_metrics_published >= 1
            {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timeout");

    let got_spc = timeout(Duration::from_secs(1), async {
        loop {
            if let Ok(ev) = events.recv().await {
                if ev.topic == TOPIC_SPC_METRICS {
                    return ev.bytes;
                }
            }
        }
    })
    .await
    .expect("spc timeout");
    assert!(!got_spc.is_empty());

    let spc_snap = bus.spc().last().expect("spc snapshot");
    assert!(spc_snap.values.iter().any(|v| v.name == "ng_rate"));

    let mes = captured.lock().unwrap().clone().expect("mes body");
    assert_eq!(mes.frame_id, 300);
    assert_eq!(mes.verdict, "NG");
    assert!(mes.defect_count >= 1);

    listener.abort();
    mock_vision.abort();
    mock_mes.abort();
}

#[test]
fn threshold_hot_update_changes_dispatch_params() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile_path = root.join("domains/industrial-inspection/profiles/line-realtime.yaml");
    let profile = ProfileStore::load(&profile_path).unwrap();
    assert_eq!(profile.params().threshold, 128);
    profile.set_threshold(200);
    assert_eq!(profile.params().threshold, 200);
}

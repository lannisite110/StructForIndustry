//! E2E: HAL frame → scheduler → mock vision sidecar → task.done published.

use std::time::Duration;

use sfi_core_bus::{
    BusConfig, CoreBus, HalFrameNotify, HalPublisher, POOL_ID_LEN, SHM_NAME_LEN, SOURCE_ID_LEN,
    SchedulerConfig, TaskScheduler, TOPIC_TASK_DONE, run_hal_listener,
};
use sfi_plugin_host::run_mock_defect_detect_sidecar;
use tempfile::tempdir;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn frame_triggers_vision_task_and_task_done() {
    let dir = tempdir().unwrap();
    let bus_socket = dir.path().join("sfi-bus.sock");
    let vision_socket = dir.path().join("vision.sock");

    let vision_path = vision_socket.clone();
    let mock = tokio::spawn(async move {
        let _ = run_mock_defect_detect_sidecar(&vision_path).await;
    });

    let mut config = BusConfig::default();
    config.socket_path = bus_socket.clone();
    config.http_addr = "127.0.0.1:0".parse().unwrap();
    config.scheduler = SchedulerConfig {
        enabled: true,
        vision_socket: vision_socket.clone(),
        task_type: "vision.detect.defect".into(),
        threshold: 128,
        plugin_name: "vision-2d".into(),
        plugin_version: "0.0.1".into(),
    };

    let scheduler = TaskScheduler::new(config.scheduler.clone());
    let sched_stats = scheduler.stats();
    let bus = CoreBus::new().with_scheduler(scheduler);
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
    assert!(bus_socket.exists());
    assert!(vision_socket.exists());

    let mut publisher = HalPublisher::connect(&bus_socket).await.expect("connect hal");

    let mut notify = HalFrameNotify {
        frame_id: 200,
        timestamp_ns: 9_876_543,
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
    notify.source_id[..11].copy_from_slice(b"synthetic-0");
    notify.pool_id[..11].copy_from_slice(b"hal.default");
    notify.shm_name[..11].copy_from_slice(b"/sfi.pool.0");

    publisher.publish(&notify).await.expect("publish");

    timeout(Duration::from_secs(3), async {
        loop {
            let frames = frame_stats.snapshot().frames_received;
            let tasks = sched_stats.snapshot().tasks_completed;
            let task_done = frame_stats.snapshot().task_done_published;
            if frames >= 1 && tasks >= 1 && task_done >= 1 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timeout waiting for vision task");

    let got_task_done = timeout(Duration::from_secs(1), async {
        loop {
            if let Ok(ev) = events.recv().await {
                if ev.topic == TOPIC_TASK_DONE {
                    return ev.bytes;
                }
            }
        }
    })
    .await
    .expect("timeout waiting for task.done event");

    let reader = capnp::serialize::read_message(
        &got_task_done[..],
        capnp::message::ReaderOptions::new(),
    )
    .expect("parse ResultEvent");
    let event = reader
        .get_root::<sfi_contracts::result_capnp::result_event::Reader>()
        .expect("root");
    assert_eq!(event.get_result().expect("result").get_task_id(), 1);

    let sched = sched_stats.snapshot();
    assert_eq!(sched.tasks_dispatched, 1);
    assert_eq!(sched.tasks_completed, 1);
    assert_eq!(sched.tasks_failed, 0);

    listener.abort();
    mock.abort();
}

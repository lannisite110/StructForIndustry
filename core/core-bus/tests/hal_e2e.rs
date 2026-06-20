//! End-to-end: synthetic HAL publisher → core-bus listener → stats.

use std::time::Duration;

use sfi_core_bus::{
    BusConfig, CoreBus, HalFrameNotify, HalPublisher, NOTIFY_SIZE, POOL_ID_LEN, SHM_NAME_LEN,
    SOURCE_ID_LEN, run_hal_listener,
};
use tempfile::tempdir;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn synthetic_hal_publishes_frame_new() {
    let dir = tempdir().unwrap();
    let socket = dir.path().join("sfi-bus.sock");

    let mut config = BusConfig::default();
    config.socket_path = socket.clone();
    config.http_addr = "127.0.0.1:0".parse().unwrap();

    let bus = CoreBus::new();
    let stats = bus.stats();

    let socket_path = socket.clone();
    let bus_for_listener = bus.clone();
    let listener = tokio::spawn(async move {
        let cfg = BusConfig {
            socket_path,
            ..Default::default()
        };
        let _ = run_hal_listener(&cfg, bus_for_listener).await;
    });

    // Wait until socket exists
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(socket.exists(), "bus socket not created");

    let mut publisher = HalPublisher::connect(&socket).await.expect("connect hal");

    let mut notify = HalFrameNotify {
        frame_id: 100,
        timestamp_ns: 1_234_567,
        sequence: 1,
        width: 640,
        height: 480,
        stride: 640,
        format: 1,
        source_id: [0; SOURCE_ID_LEN],
        pool_id: [0; POOL_ID_LEN],
        slot_index: 0,
        generation: 1,
        byte_length: 640 * 480,
        shm_name: [0; SHM_NAME_LEN],
    };
    notify.source_id[..11].copy_from_slice(b"synthetic-0");
    notify.pool_id[..11].copy_from_slice(b"hal.default");
    notify.shm_name[..11].copy_from_slice(b"/sfi.pool.0");

    publisher.publish(&notify).await.expect("publish");

    timeout(Duration::from_secs(2), async {
        loop {
            if stats.snapshot().frames_received >= 1 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timeout waiting for frame");

    let snap = stats.snapshot();
    assert_eq!(snap.frames_received, 1);
    assert_eq!(snap.last_frame_id, 100);
    assert_eq!(snap.last_timestamp_ns, 1_234_567);
    assert_eq!(NOTIFY_SIZE, 144);

    listener.abort();
}

//! Load `fake-plugin` dylib from the workspace target dir and run one task.

use std::path::PathBuf;

use capnp::message::Builder;
use capnp::serialize;
use sfi_contracts::result_capnp::{self, result_payload, ResultStatus};
use sfi_contracts::task_capnp::task;
use sfi_plugin_host::InProcessPlugin;

fn workspace_target_lib(name: &str) -> PathBuf {
    let profile = std::env::var("CARGO_PROFILE").unwrap_or_else(|_| "debug".into());

    let mut bases = Vec::new();
    if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
        bases.push(PathBuf::from(target));
    }
    if let Ok(workspace) = std::env::var("CARGO_WORKSPACE_DIR") {
        bases.push(PathBuf::from(workspace).join("target"));
    }
    bases.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));

    let fallback = bases
        .last()
        .cloned()
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"))
        .join(&profile)
        .join("deps")
        .join(name);

    for base in &bases {
        for sub in ["deps", ""] {
            let path = base.join(&profile).join(sub).join(name);
            if path.exists() {
                return path;
            }
        }
    }

    fallback
}

#[test]
fn loads_fake_plugin_and_returns_detection() {
    let lib_path = workspace_target_lib(fake_plugin::library_filename());
    assert!(
        lib_path.exists(),
        "build fake-plugin first: {}",
        lib_path.display()
    );

    let plugin = InProcessPlugin::load(&lib_path).expect("load plugin");
    assert_eq!(plugin.info().name, "fake-plugin");
    assert!(plugin
        .info()
        .capabilities
        .contains(&"vision.detect.defect".to_string()));

    let mut message = Builder::new_default();
    let mut task_builder = message.init_root::<task::Builder>();
    task_builder.set_id(42);
    task_builder.set_type("vision.detect.defect");
    task_builder.init_input().init_frame_ref().set_id(1001);

    let mut task_bytes = Vec::new();
    serialize::write_message(&mut task_bytes, &message).unwrap();

    let result_bytes = plugin.process_task(&task_bytes).expect("process task");

    let reader =
        serialize::read_message(&result_bytes[..], capnp::message::ReaderOptions::new()).unwrap();
    let result_reader = reader.get_root::<result_capnp::result::Reader>().unwrap();

    assert_eq!(result_reader.get_task_id(), 42);
    assert_eq!(result_reader.get_status().expect("status"), ResultStatus::Ok);

    let detections = match result_reader.get_payload().expect("payload").which().expect("which") {
        result_payload::WhichReader::Detections(d) => d.expect("detections"),
        other => panic!("unexpected payload: {:?}", std::mem::discriminant(&other)),
    };
    assert_eq!(detections.get_frame_id(), 1001);
    assert_eq!(detections.get_detections().expect("list").len(), 1);

    let det = detections.get_detections().expect("list").get(0);
    assert_eq!(det.get_label().expect("label"), "mock-defect");
}

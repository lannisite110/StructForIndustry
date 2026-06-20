use capnp::message::{Builder, ReaderOptions};
use capnp::serialize;
use sfi_contracts::task_capnp::{task, task_input};
use sfi_contracts::{API_VERSION_MAJOR, API_VERSION_MINOR};

#[test]
fn api_version_matches_contracts_file() {
    assert_eq!(API_VERSION_MAJOR, 0);
    assert_eq!(API_VERSION_MINOR, 0);
}

#[test]
fn roundtrip_task_message() {
    let mut message = Builder::new_default();
    let mut task_builder = message.init_root::<task::Builder>();
    task_builder.set_id(42);
    task_builder.set_type("vision.detect.defect");
    task_builder.set_correlation_id("corr-1");
    task_builder.init_input().init_frame_ref().set_id(1001);

    let mut bytes = Vec::new();
    serialize::write_message(&mut bytes, &message).expect("write task");

    let reader = serialize::read_message(&bytes[..], ReaderOptions::new()).expect("read task");
    let task_reader = reader.get_root::<task::Reader>().expect("root");
    assert_eq!(task_reader.get_id(), 42);
    assert_eq!(
        task_reader.get_type().expect("type"),
        "vision.detect.defect"
    );
    match task_reader
        .get_input()
        .expect("input")
        .which()
        .expect("which")
    {
        task_input::WhichReader::FrameRef(fr) => assert_eq!(fr.expect("fr").get_id(), 1001),
        other => panic!("unexpected input: {:?}", std::mem::discriminant(&other)),
    }
}

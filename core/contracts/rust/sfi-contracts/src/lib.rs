//! Generated Cap'n Proto modules for apiVersion 0.
//!
//! Modules are produced by `build.rs` from `../../schema/*.capnp`.

pub mod common_capnp {
    include!(concat!(env!("OUT_DIR"), "/common_capnp.rs"));
}

pub mod buffer_capnp {
    include!(concat!(env!("OUT_DIR"), "/buffer_capnp.rs"));
}

pub mod frame_capnp {
    include!(concat!(env!("OUT_DIR"), "/frame_capnp.rs"));
}

pub mod task_capnp {
    include!(concat!(env!("OUT_DIR"), "/task_capnp.rs"));
}

pub mod result_capnp {
    include!(concat!(env!("OUT_DIR"), "/result_capnp.rs"));
}

pub mod manifest_capnp {
    include!(concat!(env!("OUT_DIR"), "/manifest_capnp.rs"));
}

pub mod bus_capnp {
    include!(concat!(env!("OUT_DIR"), "/bus_capnp.rs"));
}

pub mod sfi_capnp {
    include!(concat!(env!("OUT_DIR"), "/sfi_capnp.rs"));
}

pub const API_VERSION_MAJOR: u16 = 0;
pub const API_VERSION_MINOR: u16 = 0;

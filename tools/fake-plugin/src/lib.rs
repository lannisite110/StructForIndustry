//! Mock in-process plugin implementing `abi/sfi.h`.
//!
//! Returns a single synthetic detection for any `vision.*` task type.

use std::ffi::{c_char, c_void, CStr};
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use capnp::message::{Builder, ReaderOptions};
use capnp::serialize;
use sfi_abi::{sfi_host, sfi_plugin_info, SFI_API_VERSION_MAJOR};
use sfi_contracts::common_capnp::StatusCode;
use sfi_contracts::result_capnp::{self, ResultStatus};
use sfi_contracts::task_capnp::{task, task_input};

static INITIALIZED: AtomicBool = AtomicBool::new(false);

static CAP0: &[u8] = b"vision.detect.defect\0";
static CAP1: &[u8] = b"vision.detect.mock\0";

static NAME: &[u8] = b"fake-plugin\0";
static VERSION: &[u8] = b"0.0.1\0";

struct CapTable([*const c_char; 2]);
// Capability name pointers are immutable after init.
unsafe impl Send for CapTable {}
unsafe impl Sync for CapTable {}

static CAP_TABLE: OnceLock<CapTable> = OnceLock::new();

pub fn library_filename() -> &'static str {
    if cfg!(target_os = "windows") {
        "fake_plugin.dll"
    } else if cfg!(target_os = "macos") {
        "libfake_plugin.dylib"
    } else {
        "libfake_plugin.so"
    }
}

#[no_mangle]
pub extern "C" fn sfi_init(
    host: *const sfi_host,
    out_info: *mut sfi_plugin_info,
) -> libc::c_int {
    if host.is_null() || out_info.is_null() {
        return -1;
    }

    let host_ref = unsafe { &*host };
    if host_ref.api_version_major != SFI_API_VERSION_MAJOR {
        return -2;
    }

    if let Some(log) = host_ref.log_info {
        let msg = CStr::from_bytes_with_nul(b"fake-plugin: init\0").unwrap();
        log(msg.as_ptr());
    }

    let ptrs = CAP_TABLE
        .get_or_init(|| {
            CapTable([
                CAP0.as_ptr() as *const c_char,
                CAP1.as_ptr() as *const c_char,
            ])
        })
        .0;

    unsafe {
        (*out_info).name = NAME.as_ptr() as *const c_char;
        (*out_info).version = VERSION.as_ptr() as *const c_char;
        (*out_info).capabilities = ptrs.as_ptr() as *mut *const c_char;
        (*out_info).capability_count = 2;
    }

    INITIALIZED.store(true, Ordering::SeqCst);
    0
}

#[no_mangle]
pub extern "C" fn sfi_process_task(
    task_msg: *const c_void,
    task_len: usize,
    result_msg: *mut c_void,
    result_cap: usize,
    result_len: *mut usize,
) -> libc::c_int {
    if !INITIALIZED.load(Ordering::SeqCst) {
        return -1;
    }
    if task_msg.is_null() || result_msg.is_null() || result_len.is_null() {
        return -1;
    }

    let task_bytes = unsafe { slice::from_raw_parts(task_msg as *const u8, task_len) };
    let out_buf = unsafe { slice::from_raw_parts_mut(result_msg as *mut u8, result_cap) };

    let reader = match serialize::read_message(task_bytes, ReaderOptions::new()) {
        Ok(r) => r,
        Err(_) => return -2,
    };

    let task_reader = match reader.get_root::<task::Reader>() {
        Ok(t) => t,
        Err(_) => return -3,
    };

    let task_id = task_reader.get_id();
    let frame_id = match task_reader.get_input().expect("input").which().expect("which") {
        task_input::WhichReader::FrameRef(fr) => fr.expect("frame_ref").get_id(),
        task_input::WhichReader::Frame(f) => f.expect("frame").get_id(),
        _ => 0,
    };

    let mut message = Builder::new_default();
    let mut result_builder = message.init_root::<result_capnp::result::Builder>();
    result_builder.set_task_id(task_id);
    result_builder.set_status(ResultStatus::Ok);
    result_builder.set_code(StatusCode::Ok);
    result_builder.set_message("mock detection");
    result_builder.set_plugin_name("fake-plugin");
    result_builder.set_plugin_version("0.0.1");

    let payload = result_builder.init_payload();
    let mut det_list = payload.init_detections();
    det_list.set_frame_id(frame_id);
    det_list.set_source_id("fake-source");
    let mut list = det_list.init_detections(1);
    let mut det = list.reborrow().get(0);
    det.set_class_id(1);
    det.set_label("mock-defect");
    det.set_score(0.99);
    let mut bbox = det.init_bbox();
    bbox.set_x(0.1);
    bbox.set_y(0.2);
    bbox.set_width(0.3);
    bbox.set_height(0.4);

    let mut cursor = std::io::Cursor::new(out_buf);
    if serialize::write_message(&mut cursor, &message).is_err() {
        return -4;
    }
    let written = cursor.position() as usize;
    unsafe {
        *result_len = written;
    }
    0
}

#[no_mangle]
pub extern "C" fn sfi_shutdown() {
    INITIALIZED.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;
    use sfi_abi::{SFI_API_VERSION_MAJOR, SFI_API_VERSION_MINOR};

    extern "C" fn noop_log(_msg: *const c_char) {}

    #[test]
    fn process_mock_task_via_c_abi() {
        let host = sfi_host {
            api_version_major: SFI_API_VERSION_MAJOR,
            api_version_minor: SFI_API_VERSION_MINOR,
            log_info: Some(noop_log),
            map_buffer: None,
            release_buffer: None,
            user_data: ptr::null_mut(),
        };

        let mut info = sfi_plugin_info {
            name: ptr::null(),
            version: ptr::null(),
            capabilities: ptr::null_mut(),
            capability_count: 0,
        };

        assert_eq!(sfi_init(&host, &mut info), 0);

        let mut message = Builder::new_default();
        let mut task_builder = message.init_root::<task::Builder>();
        task_builder.set_id(7);
        task_builder.set_type("vision.detect.defect");
        task_builder.init_input().init_frame_ref().set_id(99);

        let mut task_bytes = Vec::new();
        serialize::write_message(&mut task_bytes, &message).unwrap();

        let mut result_bytes = vec![0u8; 4096];
        let mut result_len = 0usize;

        let rc = sfi_process_task(
            task_bytes.as_ptr() as *const c_void,
            task_bytes.len(),
            result_bytes.as_mut_ptr() as *mut c_void,
            result_bytes.len(),
            &mut result_len,
        );
        assert_eq!(rc, 0);
        assert!(result_len > 0);

        let reader =
            serialize::read_message(&result_bytes[..result_len], ReaderOptions::new()).unwrap();
        let result_reader = reader.get_root::<result_capnp::result::Reader>().unwrap();
        assert_eq!(result_reader.get_task_id(), 7);
        assert_eq!(result_reader.get_status().expect("status"), ResultStatus::Ok);

        sfi_shutdown();
    }
}

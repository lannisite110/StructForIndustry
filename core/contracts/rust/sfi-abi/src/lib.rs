//! Manual Rust bindings for `core/contracts/abi/sfi.h` (apiVersion 0).

use std::os::raw::{c_char, c_int, c_void};

pub const SFI_API_VERSION_MAJOR: u16 = 0;
pub const SFI_API_VERSION_MINOR: u16 = 0;

#[repr(C)]
pub struct sfi_host {
    pub api_version_major: u16,
    pub api_version_minor: u16,
    pub log_info: Option<extern "C" fn(msg: *const c_char)>,
    pub map_buffer: Option<
        extern "C" fn(
            handle_msg: *const c_void,
            handle_len: usize,
            out_ptr: *mut *mut c_void,
            out_len: *mut usize,
            map_cookie: *mut *mut c_void,
        ) -> c_int,
    >,
    pub release_buffer: Option<extern "C" fn(map_cookie: *mut c_void)>,
    pub user_data: *mut c_void,
}

#[repr(C)]
pub struct sfi_plugin_info {
    pub name: *const c_char,
    pub version: *const c_char,
    pub capabilities: *mut *const c_char,
    pub capability_count: usize,
}

pub type SfiInitFn =
    unsafe extern "C" fn(host: *const sfi_host, out_info: *mut sfi_plugin_info) -> c_int;

pub type SfiProcessFn = unsafe extern "C" fn(
    task_msg: *const c_void,
    task_len: usize,
    result_msg: *mut c_void,
    result_cap: usize,
    result_len: *mut usize,
) -> c_int;

pub type SfiShutdownFn = unsafe extern "C" fn();

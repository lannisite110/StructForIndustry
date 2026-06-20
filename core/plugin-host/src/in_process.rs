use std::ffi::{c_char, c_void, CStr};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use libloading::{Library, Symbol};
use sfi_abi::{
    sfi_host, sfi_plugin_info, SfiInitFn, SfiProcessFn, SfiShutdownFn, SFI_API_VERSION_MAJOR,
    SFI_API_VERSION_MINOR,
};
use thiserror::Error;

static LOG_COUNTER: AtomicUsize = AtomicUsize::new(0);

extern "C" fn host_log_info(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    let _ = unsafe { CStr::from_ptr(msg) };
    LOG_COUNTER.fetch_add(1, Ordering::SeqCst);
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to load library: {0}")]
    Library(#[from] libloading::Error),
    #[error("sfi_init returned {0}")]
    InitFailed(i32),
    #[error("plugin reported empty name")]
    EmptyName,
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("sfi_process_task returned {0}")]
    PluginReturned(i32),
    #[error("result buffer too small")]
    BufferTooSmall,
    #[error("capnp: {0}")]
    Capnp(capnp::Error),
}

pub struct InProcessPlugin {
    _library: Library,
    shutdown_fn: SfiShutdownFn,
    process_fn: SfiProcessFn,
    info: PluginInfo,
}

impl InProcessPlugin {
    pub fn load(path: &Path) -> Result<Self, LoadError> {
        let library = unsafe { Library::new(path)? };

        let init: Symbol<SfiInitFn> = unsafe { library.get(b"sfi_init")? };

        let host = sfi_host {
            api_version_major: SFI_API_VERSION_MAJOR,
            api_version_minor: SFI_API_VERSION_MINOR,
            log_info: Some(host_log_info),
            map_buffer: None,
            release_buffer: None,
            user_data: ptr::null_mut(),
        };

        let mut raw_info = sfi_plugin_info {
            name: ptr::null(),
            version: ptr::null(),
            capabilities: ptr::null_mut(),
            capability_count: 0,
        };

        let init_rc = unsafe { init(&host, &mut raw_info) };
        if init_rc != 0 {
            return Err(LoadError::InitFailed(init_rc));
        }

        let process_fn = unsafe { *library.get(b"sfi_process_task")? };
        let shutdown_fn = unsafe { *library.get(b"sfi_shutdown")? };

        let info = read_plugin_info(&raw_info)?;

        Ok(Self {
            _library: library,
            shutdown_fn,
            process_fn,
            info,
        })
    }

    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    pub fn process_task(&self, task_bytes: &[u8]) -> Result<Vec<u8>, ProcessError> {
        let mut result_buf = vec![0u8; 64 * 1024];
        let mut result_len = 0usize;

        let rc = unsafe {
            (self.process_fn)(
                task_bytes.as_ptr() as *const c_void,
                task_bytes.len(),
                result_buf.as_mut_ptr() as *mut c_void,
                result_buf.len(),
                &mut result_len,
            )
        };

        if rc != 0 {
            return Err(ProcessError::PluginReturned(rc));
        }
        if result_len == 0 {
            return Err(ProcessError::BufferTooSmall);
        }

        result_buf.truncate(result_len);
        Ok(result_buf)
    }
}

impl Drop for InProcessPlugin {
    fn drop(&mut self) {
        unsafe { (self.shutdown_fn)() };
    }
}

fn read_plugin_info(raw: &sfi_plugin_info) -> Result<PluginInfo, LoadError> {
    if raw.name.is_null() {
        return Err(LoadError::EmptyName);
    }

    let name = unsafe { CStr::from_ptr(raw.name) }
        .to_string_lossy()
        .into_owned();
    let version = if raw.version.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(raw.version) }.to_string_lossy().into_owned()
    };

    let mut capabilities = Vec::new();
    if !raw.capabilities.is_null() && raw.capability_count > 0 {
        let caps =
            unsafe { std::slice::from_raw_parts(raw.capabilities, raw.capability_count) };
        for cap in caps {
            if !cap.is_null() {
                capabilities
                    .push(unsafe { CStr::from_ptr(*cap) }.to_string_lossy().into_owned());
            }
        }
    }

    Ok(PluginInfo {
        name,
        version,
        capabilities,
    })
}

#[cfg(test)]
pub fn host_log_count() -> usize {
    LOG_COUNTER.load(Ordering::SeqCst)
}

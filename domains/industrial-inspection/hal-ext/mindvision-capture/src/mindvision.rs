//! MindVision MVCAMSDK backend — mock (CI/Linux) + dynamic DLL on Windows.

use sfi_line_frame::{fill_frame, Gray8Layout};

#[derive(Debug, Clone)]
pub struct MindVisionConfig {
    pub device_index: u32,
    pub serial: Option<String>,
    pub width: u32,
    pub height: u32,
    pub mock: bool,
}

#[derive(Debug)]
pub struct MindVisionFrame {
    pub pixels: Vec<u8>,
}

pub trait MindVisionBackend: Send {
    fn open(config: &MindVisionConfig) -> Result<Self, String>
    where
        Self: Sized;
    fn layout(&self) -> Gray8Layout;
    fn grab(&mut self, frame_index: u64) -> Result<MindVisionFrame, String>;
}

pub struct MockMindVisionBackend {
    layout: Gray8Layout,
}

impl MindVisionBackend for MockMindVisionBackend {
    fn open(config: &MindVisionConfig) -> Result<Self, String> {
        Ok(Self {
            layout: Gray8Layout {
                width: config.width,
                height: config.height,
                stride: config.width,
            },
        })
    }

    fn layout(&self) -> Gray8Layout {
        self.layout
    }

    fn grab(&mut self, frame_index: u64) -> Result<MindVisionFrame, String> {
        let mut pixels = vec![0u8; self.layout.byte_length()];
        fill_frame(&mut pixels, frame_index, frame_index.is_multiple_of(3));
        Ok(MindVisionFrame { pixels })
    }
}

#[cfg(windows)]
mod windows_sdk {
    use super::*;
    use libloading::{Library, Symbol};
    use std::path::PathBuf;
    use std::ptr;

    const CAMERA_STATUS_SUCCESS: i32 = 0;
    const CAMERA_MEDIA_TYPE_MONO8: u32 = 17_301_505;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct tSdkCameraDevInfo {
        ac_product_series: [i8; 32],
        ac_product_name: [i8; 32],
        ac_friendly_name: [i8; 32],
        ac_link_name: [i8; 32],
        ac_driver_version: [i8; 32],
        ac_sensor_type: [i8; 32],
        ac_port_type: [i8; 32],
        ac_sn: [i8; 32],
        u_instance: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct tSdkFrameHead {
        ui_media_type: u32,
        u_bytes: u32,
        i_width: i32,
        i_height: i32,
        i_width_zoom_sw: i32,
        i_height_zoom_sw: i32,
        b_is_trigger: i32,
        ui_time_stamp: u32,
        ui_exp_time: u32,
        f_analog_gain: f32,
        i_gamma: i32,
        i_contrast: i32,
        i_saturation: i32,
        f_rgain: f32,
        f_ggain: f32,
        f_bgain: f32,
    }

    struct SdkApi {
        _lib: Library,
        camera_sdk_init: unsafe extern "system" fn(i32) -> i32,
        camera_enumerate_device: unsafe extern "system" fn(*mut tSdkCameraDevInfo, *mut i32) -> i32,
        camera_init: unsafe extern "system" fn(*mut tSdkCameraDevInfo, i32, i32, *mut i32) -> i32,
        camera_set_isp_out_format: unsafe extern "system" fn(i32, u32) -> i32,
        camera_play: unsafe extern "system" fn(i32) -> i32,
        camera_get_image_buffer:
            unsafe extern "system" fn(i32, *mut tSdkFrameHead, *mut *mut u8, u32) -> i32,
        camera_release_image_buffer: unsafe extern "system" fn(i32, *mut u8) -> i32,
        camera_un_init: unsafe extern "system" fn(i32) -> i32,
    }

    impl SdkApi {
        fn load() -> Result<Self, String> {
            let dll = dll_path();
            let lib = unsafe { Library::new(&dll) }.map_err(|e| format!("load {dll}: {e}"))?;
            unsafe {
                Ok(Self {
                    camera_sdk_init: *load_sym::<unsafe extern "system" fn(i32) -> i32>(
                        &lib,
                        "CameraSdkInit",
                    )?,
                    camera_enumerate_device: *load_sym::<
                        unsafe extern "system" fn(*mut tSdkCameraDevInfo, *mut i32) -> i32,
                    >(&lib, "CameraEnumerateDevice")?,
                    camera_init: *load_sym::<
                        unsafe extern "system" fn(
                            *mut tSdkCameraDevInfo,
                            i32,
                            i32,
                            *mut i32,
                        ) -> i32,
                    >(&lib, "CameraInit")?,
                    camera_set_isp_out_format: *load_sym::<
                        unsafe extern "system" fn(i32, u32) -> i32,
                    >(
                        &lib, "CameraSetIspOutFormat"
                    )?,
                    camera_play: *load_sym::<unsafe extern "system" fn(i32) -> i32>(
                        &lib,
                        "CameraPlay",
                    )?,
                    camera_get_image_buffer: *load_sym::<
                        unsafe extern "system" fn(
                            i32,
                            *mut tSdkFrameHead,
                            *mut *mut u8,
                            u32,
                        ) -> i32,
                    >(&lib, "CameraGetImageBuffer")?,
                    camera_release_image_buffer: *load_sym::<
                        unsafe extern "system" fn(i32, *mut u8) -> i32,
                    >(
                        &lib, "CameraReleaseImageBuffer"
                    )?,
                    camera_un_init: *load_sym::<unsafe extern "system" fn(i32) -> i32>(
                        &lib,
                        "CameraUnInit",
                    )?,
                    _lib: lib,
                })
            }
        }
    }

    unsafe fn load_sym<T>(lib: &Library, name: &str) -> Result<Symbol<T>, String> {
        lib.get(name.as_bytes())
            .map_err(|e| format!("symbol {name}: {e}"))
    }

    fn dll_path() -> PathBuf {
        if let Ok(path) = std::env::var("SFI_MINDVISION_SDK_DLL") {
            return PathBuf::from(path);
        }
        PathBuf::from(r"C:\Windows\System32\MVCAMSDK_X64.dll")
    }

    fn cstr_field(buf: &[i8]) -> String {
        let bytes: Vec<u8> = buf
            .iter()
            .map(|&c| c as u8)
            .take_while(|&b| b != 0)
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    pub struct SdkMindVisionBackend {
        api: SdkApi,
        handle: i32,
        layout: Gray8Layout,
    }

    impl SdkMindVisionBackend {
        pub fn open(config: &MindVisionConfig) -> Result<Self, String> {
            let api = SdkApi::load()?;
            let status = unsafe { (api.camera_sdk_init)(1) };
            if status != CAMERA_STATUS_SUCCESS {
                return Err(format!("CameraSdkInit failed: {status}"));
            }

            let mut count: i32 = 0;
            let status = unsafe { (api.camera_enumerate_device)(ptr::null_mut(), &mut count) };
            if status != CAMERA_STATUS_SUCCESS {
                return Err(format!("CameraEnumerateDevice(count) failed: {status}"));
            }
            if count <= 0 {
                return Err("no MindVision camera found".into());
            }

            let mut devices = vec![
                tSdkCameraDevInfo {
                    ac_product_series: [0; 32],
                    ac_product_name: [0; 32],
                    ac_friendly_name: [0; 32],
                    ac_link_name: [0; 32],
                    ac_driver_version: [0; 32],
                    ac_sensor_type: [0; 32],
                    ac_port_type: [0; 32],
                    ac_sn: [0; 32],
                    u_instance: 0,
                };
                count as usize
            ];

            let status = unsafe { (api.camera_enumerate_device)(devices.as_mut_ptr(), &mut count) };
            if status != CAMERA_STATUS_SUCCESS {
                return Err(format!("CameraEnumerateDevice failed: {status}"));
            }

            let device = select_device(&devices, count as usize, config)?;
            let mut handle: i32 = 0;
            let mut dev = device;
            let status = unsafe { (api.camera_init)(&mut dev, -1, -1, &mut handle) };
            if status != CAMERA_STATUS_SUCCESS {
                return Err(format!("CameraInit failed: {status}"));
            }

            let status =
                unsafe { (api.camera_set_isp_out_format)(handle, CAMERA_MEDIA_TYPE_MONO8) };
            if status != CAMERA_STATUS_SUCCESS {
                let _ = unsafe { (api.camera_un_init)(handle) };
                return Err(format!("CameraSetIspOutFormat MONO8 failed: {status}"));
            }

            let status = unsafe { (api.camera_play)(handle) };
            if status != CAMERA_STATUS_SUCCESS {
                let _ = unsafe { (api.camera_un_init)(handle) };
                return Err(format!("CameraPlay failed: {status}"));
            }

            let sn = cstr_field(&device.ac_sn);
            let name = cstr_field(&device.ac_friendly_name);
            tracing::info!(sn = %sn, name = %name, handle, "MindVision camera opened");

            Ok(Self {
                api,
                handle,
                layout: Gray8Layout {
                    width: config.width.max(1),
                    height: config.height.max(1),
                    stride: config.width.max(1),
                },
            })
        }
    }

    fn select_device(
        devices: &[tSdkCameraDevInfo],
        count: usize,
        config: &MindVisionConfig,
    ) -> Result<tSdkCameraDevInfo, String> {
        if let Some(ref want_sn) = config.serial {
            for dev in &devices[..count] {
                if cstr_field(&dev.ac_sn) == *want_sn {
                    return Ok(*dev);
                }
            }
            return Err(format!("camera serial {want_sn} not found"));
        }
        let idx = config.device_index as usize;
        devices
            .get(idx)
            .copied()
            .ok_or_else(|| format!("device index {idx} out of range (found {count})"))
    }

    impl MindVisionBackend for SdkMindVisionBackend {
        fn open(config: &MindVisionConfig) -> Result<Self, String> {
            SdkMindVisionBackend::open(config)
        }

        fn layout(&self) -> Gray8Layout {
            self.layout
        }

        fn grab(&mut self, _frame_index: u64) -> Result<MindVisionFrame, String> {
            let mut head = tSdkFrameHead::default();
            let mut buffer: *mut u8 = ptr::null_mut();
            let status = unsafe {
                (self.api.camera_get_image_buffer)(self.handle, &mut head, &mut buffer, 1000)
            };
            if status != CAMERA_STATUS_SUCCESS {
                return Err(format!("CameraGetImageBuffer failed: {status}"));
            }
            if buffer.is_null() {
                return Err("CameraGetImageBuffer returned null".into());
            }

            let w = head.i_width.max(0) as usize;
            let h = head.i_height.max(0) as usize;
            let len = head.u_bytes as usize;
            let pixels = unsafe { std::slice::from_raw_parts(buffer, len.min(w * h)) }.to_vec();
            let status = unsafe { (self.api.camera_release_image_buffer)(self.handle, buffer) };
            if status != CAMERA_STATUS_SUCCESS {
                return Err(format!("CameraReleaseImageBuffer failed: {status}"));
            }

            self.layout = Gray8Layout {
                width: w as u32,
                height: h as u32,
                stride: w as u32,
            };
            Ok(MindVisionFrame { pixels })
        }
    }

    impl Drop for SdkMindVisionBackend {
        fn drop(&mut self) {
            let status = unsafe { (self.api.camera_un_init)(self.handle) };
            if status != CAMERA_STATUS_SUCCESS {
                tracing::warn!(status, "CameraUnInit failed");
            }
        }
    }
}

#[cfg(not(windows))]
pub struct SdkMindVisionBackend;

#[cfg(not(windows))]
impl MindVisionBackend for SdkMindVisionBackend {
    fn open(_config: &MindVisionConfig) -> Result<Self, String> {
        Err(
            "MindVision SDK requires Windows — set SFI_MINDVISION_MOCK=1 or run on Windows with MVCAMSDK_X64.dll"
                .into(),
        )
    }

    fn layout(&self) -> Gray8Layout {
        Gray8Layout::default()
    }

    fn grab(&mut self, _frame_index: u64) -> Result<MindVisionFrame, String> {
        Err("MindVision SDK not available on this platform".into())
    }
}

#[cfg(windows)]
use windows_sdk::SdkMindVisionBackend;

pub fn open_backend(config: &MindVisionConfig) -> Result<Box<dyn MindVisionBackend>, String> {
    if config.mock {
        Ok(Box::new(MockMindVisionBackend::open(config)?))
    } else {
        Ok(Box::new(SdkMindVisionBackend::open(config)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_grab_returns_pixels() {
        let cfg = MindVisionConfig {
            device_index: 0,
            serial: None,
            width: 64,
            height: 48,
            mock: true,
        };
        let mut backend = MockMindVisionBackend::open(&cfg).unwrap();
        let frame = backend.grab(1).unwrap();
        assert_eq!(frame.pixels.len(), 64 * 48);
    }
}

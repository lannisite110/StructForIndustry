//! Shared frame source (synthetic or V4L2) for PLC trigger gateways.

use sfi_line_frame::{fill_frame, map_shm, Gray8Layout};

#[cfg(target_os = "linux")]
use sfi_v4l2::{Camera, CaptureConfig};

pub struct FrameSource {
    mmap: memmap2::MmapMut,
    layout: Gray8Layout,
    synthetic: bool,
    #[cfg(target_os = "linux")]
    camera: Option<Camera>,
}

impl FrameSource {
    pub fn open(shm_name: &str) -> std::io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            if let Ok(device) = std::env::var("SFI_V4L2_DEVICE") {
                if std::path::Path::new(&device).exists() {
                    let width: u32 = std::env::var("SFI_V4L2_WIDTH")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(640);
                    let height: u32 = std::env::var("SFI_V4L2_HEIGHT")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(480);
                    let config = CaptureConfig {
                        device,
                        width,
                        height,
                    };
                    let camera = Camera::open(&config)?;
                    let layout = camera.layout();
                    let byte_len = layout.byte_length();
                    let mmap = map_shm(shm_name, byte_len)?;
                    return Ok(Self {
                        mmap,
                        layout,
                        synthetic: false,
                        camera: Some(camera),
                    });
                }
            }
        }

        let layout = Gray8Layout::default();
        let byte_len = layout.byte_length();
        Ok(Self {
            mmap: map_shm(shm_name, byte_len)?,
            layout,
            synthetic: true,
            #[cfg(target_os = "linux")]
            camera: None,
        })
    }

    pub fn uses_v4l2(&self) -> bool {
        !self.synthetic
    }

    pub fn fill_and_layout(&mut self, frame_id: u64) -> std::io::Result<(Gray8Layout, u64)> {
        if self.synthetic {
            fill_frame(&mut self.mmap, frame_id, true);
            let byte_len = self.layout.byte_length() as u64;
            Ok((self.layout, byte_len))
        } else {
            #[cfg(target_os = "linux")]
            {
                let camera = self
                    .camera
                    .as_mut()
                    .ok_or_else(|| std::io::Error::other("v4l2 camera not open"))?;
                let frame = camera.capture_one()?;
                let n = frame.pixels.len().min(self.mmap.len());
                self.mmap[..n].copy_from_slice(&frame.pixels[..n]);
                let byte_len = self.layout.byte_length() as u64;
                Ok((self.layout, byte_len))
            }
            #[cfg(not(target_os = "linux"))]
            {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "V4L2 requires Linux",
                ))
            }
        }
    }
}

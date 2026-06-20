//! GigE / GenICam backend abstraction (Phase 3 scaffold).

use sfi_line_frame::{fill_frame, Gray8Layout};

#[derive(Debug, Clone)]
pub struct GigEConfig {
    pub device_ip: String,
    pub width: u32,
    pub height: u32,
    pub mock: bool,
}

#[derive(Debug)]
pub struct GigEFrame {
    pub pixels: Vec<u8>,
}

/// Trait for vendor SDK / GenICam implementations (Basler, Hikvision, …).
pub trait GigEBackend: Send {
    fn open(config: &GigEConfig) -> Result<Self, String>
    where
        Self: Sized;
    fn layout(&self) -> Gray8Layout;
    fn grab(&mut self, frame_index: u64) -> Result<GigEFrame, String>;
}

/// Mock backend — deterministic pattern for CI and lab without hardware.
pub struct MockGigEBackend {
    layout: Gray8Layout,
}

impl GigEBackend for MockGigEBackend {
    fn open(config: &GigEConfig) -> Result<Self, String> {
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

    fn grab(&mut self, frame_index: u64) -> Result<GigEFrame, String> {
        let mut pixels = vec![0u8; self.layout.byte_length()];
        fill_frame(&mut pixels, frame_index, frame_index % 3 == 0);
        Ok(GigEFrame { pixels })
    }
}

/// Placeholder for real GigE Vision / GenICam SDK integration.
pub struct SdkGigEBackend {
    _layout: Gray8Layout,
    _device_ip: String,
}

impl GigEBackend for SdkGigEBackend {
    fn open(config: &GigEConfig) -> Result<Self, String> {
        Err(format!(
            "GigE SDK not linked: device {} — set SFI_GIGE_MOCK=1 or integrate vendor SDK in gige_capture::SdkGigEBackend",
            config.device_ip
        ))
    }

    fn layout(&self) -> Gray8Layout {
        self._layout
    }

    fn grab(&mut self, _frame_index: u64) -> Result<GigEFrame, String> {
        Err("GigE SDK not implemented".into())
    }
}

pub fn open_backend(config: &GigEConfig) -> Result<Box<dyn GigEBackend>, String> {
    if config.mock {
        Ok(Box::new(MockGigEBackend::open(config)?))
    } else {
        Ok(Box::new(SdkGigEBackend::open(config)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_grab_returns_pixels() {
        let cfg = GigEConfig {
            device_ip: "192.168.0.99".into(),
            width: 64,
            height: 48,
            mock: true,
        };
        let mut backend = MockGigEBackend::open(&cfg).unwrap();
        let frame = backend.grab(1).unwrap();
        assert_eq!(frame.pixels.len(), 64 * 48);
    }
}

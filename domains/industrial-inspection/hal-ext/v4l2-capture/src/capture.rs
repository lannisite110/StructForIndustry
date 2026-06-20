//! V4L2 device open, format negotiation, and frame dequeue (Linux ioctl, no bindgen).

use sfi_line_frame::{gray8_to_strided, yuyv_to_gray8, Gray8Layout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    Yuyv,
}

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub device: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct CapturedFrame {
    pub pixels: Vec<u8>,
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::fs::OpenOptions;
    use std::io;
    use std::os::unix::io::AsRawFd;

    use memmap2::{Mmap, MmapOptions};
    use nix::ioctl_readwrite;
    use nix::ioctl_write_ptr;

    const V4L2_BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
    const V4L2_MEMORY_MMAP: u32 = 1;
    const V4L2_FIELD_NONE: u32 = 1;

    const PIX_GREY: u32 = 0x5945_5247; // 'GREY'
    const PIX_YUYV: u32 = 0x5659_5559; // 'YUYV'

    #[repr(C)]
    struct v4l2_pix_format {
        width: u32,
        height: u32,
        pixelformat: u32,
        field: u32,
        bytesperline: u32,
        sizeimage: u32,
        colorspace: u32,
        priv_: u32,
        flags: u32,
        ycbcr_enc: u32,
        quantization: u32,
        xfer_func: u32,
    }

    #[repr(C)]
    struct v4l2_format {
        type_: u32,
        fmt: v4l2_pix_format,
    }

    #[repr(C)]
    struct v4l2_requestbuffers {
        count: u32,
        type_: u32,
        memory: u32,
        capabilities: u32,
        reserved: [u32; 1],
    }

    #[repr(C)]
    struct v4l2_timecode {
        type_: u32,
        flags: u32,
        frames: u8,
        seconds: u8,
        minutes: u8,
        hours: u8,
        userbits: [u8; 4],
    }

    #[repr(C)]
    union v4l2_buffer_m {
        offset: u32,
        userptr: u64,
        planes: u64,
        fd: i32,
    }

    #[repr(C)]
    struct v4l2_buffer {
        index: u32,
        type_: u32,
        bytesused: u32,
        flags: u32,
        field: u32,
        timestamp_tv_sec: i64,
        timestamp_tv_usec: i64,
        timecode: v4l2_timecode,
        sequence: u32,
        memory: u32,
        m: v4l2_buffer_m,
        length: u32,
        reserved2: u32,
        request_fd: i32,
    }

    ioctl_readwrite!(vidioc_s_fmt, b'V', 5, v4l2_format);
    ioctl_readwrite!(vidioc_g_fmt, b'V', 4, v4l2_format);
    ioctl_readwrite!(vidioc_reqbufs, b'V', 8, v4l2_requestbuffers);
    ioctl_readwrite!(vidioc_querybuf, b'V', 9, v4l2_buffer);
    ioctl_readwrite!(vidioc_qbuf, b'V', 15, v4l2_buffer);
    ioctl_readwrite!(vidioc_dqbuf, b'V', 17, v4l2_buffer);
    ioctl_write_ptr!(vidioc_streamon, b'V', 18, i32);
    ioctl_write_ptr!(vidioc_streamoff, b'V', 19, i32);

    struct MappedBuffer {
        _mmap: Mmap,
        length: usize,
    }

    pub struct V4l2Camera {
        _file: std::fs::File,
        fd: i32,
        format: PixelFormat,
        layout: Gray8Layout,
        buffers: Vec<MappedBuffer>,
    }

    impl V4l2Camera {
        pub fn open(config: &CaptureConfig) -> io::Result<Self> {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&config.device)?;
            let fd = file.as_raw_fd();

            let (format, layout) = negotiate_format(fd, config.width, config.height)?;
            let buffers = init_mmap_buffers(fd, layout)?;
            stream_on(fd)?;

            tracing::info!(
                device = %config.device,
                width = layout.width,
                height = layout.height,
                format = ?format,
                "v4l2 capture ready"
            );

            Ok(Self {
                _file: file,
                fd,
                format,
                layout,
                buffers,
            })
        }

        pub fn layout(&self) -> Gray8Layout {
            self.layout
        }

        pub fn capture_one(&mut self) -> io::Result<CapturedFrame> {
            let mut buf = v4l2_buffer {
                index: 0,
                type_: V4L2_BUF_TYPE_VIDEO_CAPTURE,
                bytesused: 0,
                flags: 0,
                field: V4L2_FIELD_NONE,
                timestamp_tv_sec: 0,
                timestamp_tv_usec: 0,
                timecode: v4l2_timecode {
                    type_: 0,
                    flags: 0,
                    frames: 0,
                    seconds: 0,
                    minutes: 0,
                    hours: 0,
                    userbits: [0; 4],
                },
                sequence: 0,
                memory: V4L2_MEMORY_MMAP,
                m: v4l2_buffer_m { offset: 0 },
                length: 0,
                reserved2: 0,
                request_fd: 0,
            };

            unsafe {
                vidioc_dqbuf(self.fd, &mut buf)
                    .map_err(|e| io::Error::other(format!("VIDIOC_DQBUF: {e}")))?;
            }

            let mapped = &self.buffers[buf.index as usize];
            let used = if buf.bytesused > 0 {
                buf.bytesused as usize
            } else {
                mapped.length
            };
            let src = &mapped._mmap[..used.min(mapped.length)];

            let mut pixels = vec![0u8; self.layout.byte_length()];
            match self.format {
                PixelFormat::Gray8 => gray8_to_strided(src, self.layout, &mut pixels),
                PixelFormat::Yuyv => yuyv_to_gray8(src, self.layout, &mut pixels),
            }

            unsafe {
                vidioc_qbuf(self.fd, &mut buf)
                    .map_err(|e| io::Error::other(format!("VIDIOC_QBUF: {e}")))?;
            }

            Ok(CapturedFrame { pixels })
        }
    }

    impl Drop for V4l2Camera {
        fn drop(&mut self) {
            let typ = V4L2_BUF_TYPE_VIDEO_CAPTURE as i32;
            let _ = unsafe { vidioc_streamoff(self.fd, &typ) };
        }
    }

    fn negotiate_format(fd: i32, req_w: u32, req_h: u32) -> io::Result<(PixelFormat, Gray8Layout)> {
        if let Ok(layout) = try_format(fd, req_w, req_h, PIX_GREY) {
            return Ok((PixelFormat::Gray8, layout));
        }
        if let Ok(layout) = try_format(fd, req_w, req_h, PIX_YUYV) {
            return Ok((PixelFormat::Yuyv, layout));
        }
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "device does not support GREY or YUYV at requested resolution",
        ))
    }

    fn try_format(fd: i32, width: u32, height: u32, pixelformat: u32) -> io::Result<Gray8Layout> {
        let mut fmt = v4l2_format {
            type_: V4L2_BUF_TYPE_VIDEO_CAPTURE,
            fmt: v4l2_pix_format {
                width,
                height,
                pixelformat,
                field: V4L2_FIELD_NONE,
                bytesperline: 0,
                sizeimage: 0,
                colorspace: 0,
                priv_: 0,
                flags: 0,
                ycbcr_enc: 0,
                quantization: 0,
                xfer_func: 0,
            },
        };
        unsafe {
            vidioc_s_fmt(fd, &mut fmt)
                .map_err(|e| io::Error::other(format!("VIDIOC_S_FMT: {e}")))?;
            vidioc_g_fmt(fd, &mut fmt)
                .map_err(|e| io::Error::other(format!("VIDIOC_G_FMT: {e}")))?;
        }
        let pix = fmt.fmt;
        let stride = if pix.bytesperline > 0 {
            pix.bytesperline
        } else {
            pix.width
        };
        Ok(Gray8Layout {
            width: pix.width,
            height: pix.height,
            stride,
        })
    }

    fn init_mmap_buffers(fd: i32, _layout: Gray8Layout) -> io::Result<Vec<MappedBuffer>> {
        let mut req = v4l2_requestbuffers {
            count: 2,
            type_: V4L2_BUF_TYPE_VIDEO_CAPTURE,
            memory: V4L2_MEMORY_MMAP,
            capabilities: 0,
            reserved: [0],
        };
        unsafe {
            vidioc_reqbufs(fd, &mut req)
                .map_err(|e| io::Error::other(format!("VIDIOC_REQBUFS: {e}")))?;
        }
        if req.count == 0 {
            return Err(io::Error::other("VIDIOC_REQBUFS returned zero buffers"));
        }

        let mut buffers = Vec::with_capacity(req.count as usize);
        for index in 0..req.count {
            let mut buf = v4l2_buffer {
                index,
                type_: V4L2_BUF_TYPE_VIDEO_CAPTURE,
                bytesused: 0,
                flags: 0,
                field: V4L2_FIELD_NONE,
                timestamp_tv_sec: 0,
                timestamp_tv_usec: 0,
                timecode: v4l2_timecode {
                    type_: 0,
                    flags: 0,
                    frames: 0,
                    seconds: 0,
                    minutes: 0,
                    hours: 0,
                    userbits: [0; 4],
                },
                sequence: 0,
                memory: V4L2_MEMORY_MMAP,
                m: v4l2_buffer_m { offset: 0 },
                length: 0,
                reserved2: 0,
                request_fd: 0,
            };
            unsafe {
                vidioc_querybuf(fd, &mut buf)
                    .map_err(|e| io::Error::other(format!("VIDIOC_QUERYBUF: {e}")))?;
            }

            let mmap = unsafe {
                MmapOptions::new()
                    .len(buf.length as usize)
                    .offset(buf.m.offset as u64)
                    .map(fd)
            }
            .map_err(|e| io::Error::other(e.to_string()))?;

            unsafe {
                vidioc_qbuf(fd, &mut buf)
                    .map_err(|e| io::Error::other(format!("VIDIOC_QBUF: {e}")))?;
            }

            buffers.push(MappedBuffer {
                _mmap: mmap,
                length: buf.length as usize,
            });
        }
        Ok(buffers)
    }

    fn stream_on(fd: i32) -> io::Result<()> {
        let typ = V4L2_BUF_TYPE_VIDEO_CAPTURE as i32;
        unsafe {
            vidioc_streamon(fd, &typ)
                .map(|_| ())
                .map_err(|e| io::Error::other(format!("VIDIOC_STREAMON: {e}")))?;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub use linux::V4l2Camera as Camera;

#[cfg(not(target_os = "linux"))]
pub struct V4l2Camera;

#[cfg(not(target_os = "linux"))]
impl V4l2Camera {
    pub fn open(_config: &CaptureConfig) -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "V4L2 capture is only supported on Linux",
        ))
    }

    pub fn layout(&self) -> Gray8Layout {
        Gray8Layout::default()
    }

    pub fn capture_one(&mut self) -> std::io::Result<CapturedFrame> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "V4L2 capture is only supported on Linux",
        ))
    }
}

#[cfg(not(target_os = "linux"))]
pub use V4l2Camera as Camera;

pub fn device_available(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

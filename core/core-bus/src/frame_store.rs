//! Archive gray8 frames for traceability (NG/audit retention).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::hal_ipc::HalFrameNotify;
use crate::profile::ComplianceSection;
use sfi_plugin_host::shm_gray8;

#[derive(Clone)]
pub struct FrameArchive {
    dir: PathBuf,
    retain_days: u32,
}

impl FrameArchive {
    pub fn from_compliance(section: &ComplianceSection) -> std::io::Result<Self> {
        let dir = frame_dir();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            retain_days: section.retain_results_days.max(1),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Copy shm payload to `{dir}/{frame_id}-{ts}.gray8`; returns relative path for API.
    pub fn archive(&self, notify: &HalFrameNotify) -> Option<String> {
        let pixels = shm_gray8::read_gray8(notify.shm_name_str(), notify.byte_length, 0).ok()?;
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = format!("{}-{}.gray8", notify.frame_id, ts);
        let path = self.dir.join(&name);
        fs::write(&path, &pixels).ok()?;
        self.prune_old();
        Some(name)
    }

    fn prune_old(&self) {
        let max_age = Duration::from_secs(self.retain_days as u64 * 86400);
        let now = SystemTime::now();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age > max_age {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

pub fn frame_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SFI_DATA_DIR") {
        return PathBuf::from(dir).join("frames");
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime).join("sfi-frames");
    }
    PathBuf::from("/tmp/sfi-frames")
}

/// Write audit-policy path hint file when compliance policy is set.
pub fn touch_policy_marker(policy_rel: &str) -> std::io::Result<()> {
    let marker = frame_dir().join(".audit-policy");
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(marker)?;
    writeln!(f, "{policy_rel}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ComplianceSection;
    use sfi_plugin_host::shm_gray8;
    use tempfile::tempdir;

    #[test]
    fn archives_frame_bytes() {
        let dir = tempdir().unwrap();
        std::env::set_var("SFI_DATA_DIR", dir.path());
        let path = dir.path().join("frame.raw");
        shm_gray8::write_test_pattern(&path, 8, 8, false).unwrap();
        let mut notify = crate::hal_ipc::HalFrameNotify {
            frame_id: 99,
            timestamp_ns: 0,
            sequence: 0,
            width: 8,
            height: 8,
            stride: 8,
            format: 1,
            source_id: [0; crate::hal_ipc::SOURCE_ID_LEN],
            pool_id: [0; crate::hal_ipc::POOL_ID_LEN],
            slot_index: 0,
            generation: 1,
            byte_length: 64,
            shm_name: [0; crate::hal_ipc::SHM_NAME_LEN],
        };
        let s = path.to_string_lossy();
        notify.shm_name[..s.len()].copy_from_slice(s.as_bytes());

        let archive = FrameArchive::from_compliance(&ComplianceSection {
            retain_results_days: 90,
            ..Default::default()
        })
        .unwrap();
        let rel = archive.archive(&notify).expect("archive");
        assert!(rel.contains("99-"));
        assert!(dir.path().join("frames").join(&rel).exists());
        std::env::remove_var("SFI_DATA_DIR");
    }
}

//! Append-only JSONL audit log for config / recipe changes.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::profile::ComplianceSection;

#[derive(Clone)]
pub struct AuditLog {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl AuditLog {
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        if !path.exists() {
            OpenOptions::new().create(true).write(true).open(&path)?;
        }
        Ok(Self {
            path,
            lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn from_compliance(section: &ComplianceSection) -> std::io::Result<Self> {
        let path = section
            .audit_log_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(default_audit_path);
        Self::open(path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record(&self, action: &str, detail: &str) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = serde_json::json!({
            "ts": ts,
            "action": action,
            "detail": detail,
        });
        let _guard = self.lock.lock().expect("audit lock");
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(file, "{line}");
        }
    }
}

pub fn default_audit_path() -> PathBuf {
    if let Ok(dir) = std::env::var("SFI_DATA_DIR") {
        return PathBuf::from(dir).join("audit.log");
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime).join("sfi-audit.log");
    }
    PathBuf::from("/tmp/sfi-audit.log")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn appends_jsonl_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("audit.log");
        let log = AuditLog::open(&path).unwrap();
        log.record("threshold.patch", "128 -> 140");
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("threshold.patch"));
        assert!(text.contains("128 -> 140"));
    }
}

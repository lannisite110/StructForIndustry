//! Append-only JSONL persistence for SPC trend history.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::spc::SpcSnapshot;

#[derive(Debug, thiserror::Error)]
pub enum SpcStoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct SpcStore {
    path: PathBuf,
    capacity: usize,
    inner: Arc<Mutex<SpcStoreInner>>,
}

struct SpcStoreInner {
    trend: Vec<SpcSnapshot>,
}

impl SpcStore {
    pub fn open(path: impl AsRef<Path>, capacity: usize) -> Result<Self, SpcStoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        if !path.exists() {
            File::create(&path)?;
        }

        let trend = load_tail(&path, capacity)?;
        Ok(Self {
            path,
            capacity: capacity.max(1),
            inner: Arc::new(Mutex::new(SpcStoreInner { trend })),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, snapshot: &SpcSnapshot) -> Result<(), SpcStoreError> {
        let line = serde_json::to_string(snapshot)?;
        {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            writeln!(file, "{line}")?;
        }
        let mut inner = self.inner.lock().expect("spc store lock");
        inner.trend.push(snapshot.clone());
        while inner.trend.len() > self.capacity {
            inner.trend.remove(0);
        }
        Ok(())
    }

    pub fn trend(&self, limit: usize) -> Vec<SpcSnapshot> {
        let inner = self.inner.lock().expect("spc store lock");
        let n = limit.min(inner.trend.len());
        inner.trend[inner.trend.len() - n..].to_vec()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("spc store lock").trend.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn load_tail(path: &Path, capacity: usize) -> Result<Vec<SpcSnapshot>, SpcStoreError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str::<SpcSnapshot>(&line)?);
    }
    let start = rows.len().saturating_sub(capacity);
    Ok(rows[start..].to_vec())
}

pub fn default_store_path() -> PathBuf {
    if let Ok(dir) = std::env::var("SFI_DATA_DIR") {
        return PathBuf::from(dir).join("spc-trend.jsonl");
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime).join("sfi-spc-trend.jsonl");
    }
    PathBuf::from("/tmp/sfi-spc-trend.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spc::SpcMetricValue;
    use tempfile::tempdir;

    fn snap(id: u64) -> SpcSnapshot {
        SpcSnapshot {
            frame_id: id,
            published_at_ns: id,
            values: vec![SpcMetricValue {
                name: "ng_rate".into(),
                value: 0.5,
                unit: "ratio".into(),
            }],
        }
    }

    #[test]
    fn append_and_reload_tail() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trend.jsonl");
        let store = SpcStore::open(&path, 100).unwrap();
        store.append(&snap(1)).unwrap();
        store.append(&snap(2)).unwrap();
        assert_eq!(store.trend(10).len(), 2);

        let reloaded = SpcStore::open(&path, 100).unwrap();
        assert_eq!(reloaded.trend(10).len(), 2);
        assert_eq!(reloaded.trend(10)[1].frame_id, 2);
    }
}

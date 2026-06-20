//! Recent inspection results for HTTP / UI.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::mes::InspectionReport;

const DEFAULT_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct ResultStore {
    inner: Arc<Mutex<VecDeque<InspectionReport>>>,
    capacity: usize,
}

impl ResultStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            capacity: capacity.max(1),
        }
    }

    pub fn push(&self, report: InspectionReport) {
        let mut q = self.inner.lock().expect("results lock");
        q.push_front(report);
        while q.len() > self.capacity {
            q.pop_back();
        }
    }

    pub fn recent(&self, limit: usize) -> Vec<InspectionReport> {
        let q = self.inner.lock().expect("results lock");
        q.iter().take(limit).cloned().collect()
    }

    pub fn last(&self) -> Option<InspectionReport> {
        self.inner.lock().expect("results lock").front().cloned()
    }
}

impl Default for ResultStore {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

use std::sync::Mutex;

use agentdeck_core::{ProcessInfo, ProcessSource};

/// An in-memory process list that tests can mutate between calls to
/// [`ProcessSource::list`].
#[derive(Debug, Default)]
pub struct MockProcessSource {
    inner: Mutex<Vec<ProcessInfo>>,
}

impl MockProcessSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the entire visible process set.
    pub fn set(&self, processes: Vec<ProcessInfo>) {
        let mut guard = self.inner.lock().expect("mock process mutex poisoned");
        *guard = processes;
    }

    /// Append one process to the visible set.
    pub fn push(&self, process: ProcessInfo) {
        let mut guard = self.inner.lock().expect("mock process mutex poisoned");
        guard.push(process);
    }

    /// Drop a process from the visible set by PID. No-op when absent.
    pub fn remove_pid(&self, pid: u32) {
        let mut guard = self.inner.lock().expect("mock process mutex poisoned");
        guard.retain(|p| p.pid != pid);
    }
}

impl ProcessSource for MockProcessSource {
    fn list(&self) -> Vec<ProcessInfo> {
        let mut list = self
            .inner
            .lock()
            .expect("mock process mutex poisoned")
            .clone();
        list.sort_by_key(|p| p.pid);
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn sample(pid: u32, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid: None,
            name: name.into(),
            cmdline: vec![name.into()],
            cwd: None,
            started_at: Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap(),
        }
    }

    #[test]
    fn list_is_sorted_by_pid() {
        let src = MockProcessSource::new();
        src.set(vec![sample(20, "aider"), sample(10, "codex")]);
        let list = src.list();
        assert_eq!(list[0].pid, 10);
        assert_eq!(list[1].pid, 20);
    }

    #[test]
    fn push_appends() {
        let src = MockProcessSource::new();
        src.set(vec![sample(10, "a")]);
        src.push(sample(20, "b"));
        assert_eq!(src.list().len(), 2);
    }

    #[test]
    fn remove_pid_drops_matching() {
        let src = MockProcessSource::new();
        src.set(vec![sample(10, "a"), sample(20, "b")]);
        src.remove_pid(10);
        let list = src.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].pid, 20);
    }

    #[test]
    fn remove_unknown_pid_is_noop() {
        let src = MockProcessSource::new();
        src.set(vec![sample(10, "a")]);
        src.remove_pid(999);
        assert_eq!(src.list().len(), 1);
    }
}

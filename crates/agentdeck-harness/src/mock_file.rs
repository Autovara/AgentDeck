use std::collections::HashSet;
use std::sync::Mutex;

use agentdeck_core::{FileEvent, FileWatcher};

/// An in-memory file watcher whose events are enqueued by tests.
#[derive(Debug, Default)]
pub struct MockFileWatcher {
    watched: Mutex<HashSet<String>>,
    queue: Mutex<Vec<FileEvent>>,
}

impl MockFileWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue an event. Tests typically enqueue events whose path
    /// matches one of the watched paths.
    pub fn enqueue(&self, event: FileEvent) {
        let mut guard = self.queue.lock().expect("mock file mutex poisoned");
        guard.push(event);
    }

    /// True if the path is currently being watched.
    pub fn is_watching(&self, path: &str) -> bool {
        self.watched
            .lock()
            .expect("mock file mutex poisoned")
            .contains(path)
    }
}

impl FileWatcher for MockFileWatcher {
    fn watch(&self, path: &str) {
        let mut guard = self.watched.lock().expect("mock file mutex poisoned");
        guard.insert(path.to_string());
    }

    fn unwatch(&self, path: &str) {
        let mut guard = self.watched.lock().expect("mock file mutex poisoned");
        guard.remove(path);
    }

    fn drain(&self) -> Vec<FileEvent> {
        let mut guard = self.queue.lock().expect("mock file mutex poisoned");
        std::mem::take(&mut *guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentdeck_core::FileEventKind;
    use chrono::{TimeZone, Utc};

    fn at(s: i32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, s as u32).unwrap()
    }

    #[test]
    fn watch_and_unwatch_round_trip() {
        let w = MockFileWatcher::new();
        w.watch("/tmp/x");
        assert!(w.is_watching("/tmp/x"));
        w.unwatch("/tmp/x");
        assert!(!w.is_watching("/tmp/x"));
    }

    #[test]
    fn drain_returns_enqueued_events_then_clears() {
        let w = MockFileWatcher::new();
        w.enqueue(FileEvent {
            path: "/tmp/x".into(),
            kind: FileEventKind::Created,
            at: at(0),
        });
        w.enqueue(FileEvent {
            path: "/tmp/x".into(),
            kind: FileEventKind::Modified,
            at: at(1),
        });
        let first = w.drain();
        assert_eq!(first.len(), 2);
        let second = w.drain();
        assert!(second.is_empty());
    }

    #[test]
    fn unwatch_unknown_path_is_noop() {
        let w = MockFileWatcher::new();
        w.unwatch("/tmp/nonexistent");
        assert!(!w.is_watching("/tmp/nonexistent"));
    }
}

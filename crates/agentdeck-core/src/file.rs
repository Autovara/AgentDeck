use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Kind of file event observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileEventKind {
    Created,
    Modified,
    Removed,
}

/// A file event delivered to subscribers of a [`FileWatcher`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEvent {
    pub path: String,
    pub kind: FileEventKind,
    pub at: DateTime<Utc>,
}

/// Watches a set of paths and yields file events.
///
/// Adapters subscribe to specific paths via [`FileWatcher::watch`]. The
/// watcher must deliver events in the order they occurred; tied events
/// may be delivered in any order. The monitor polls
/// [`FileWatcher::drain`] on every scan tick.
///
/// All methods take `&self`; implementations use interior mutability.
/// This allows multiple callers (the monitor, tests injecting events)
/// to share one watcher through `Arc<dyn FileWatcher>` without locking.
pub trait FileWatcher: Send + Sync {
    /// Begin watching the given path. Repeated calls with the same path
    /// are allowed and have no extra effect.
    fn watch(&self, path: &str);

    /// Stop watching the given path. Calling with an unknown path is a
    /// no-op.
    fn unwatch(&self, path: &str);

    /// Drain queued events. Returns an empty vector when there is
    /// nothing to deliver.
    fn drain(&self) -> Vec<FileEvent>;
}

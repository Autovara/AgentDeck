//! In-memory "Pause Alerts" toggle for the tray surface.
//!
//! When paused, the tray scheduler skips desktop notifications for
//! newly-created urgent attention items. The flag is **session-scoped**
//! in the alpha: a fresh process always starts with alerts enabled.
//! Persisting the state to the `settings` table is build-plan §15 step
//! 18 work and lands later.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Tauri-managed state wrapping an atomic flag. Cloneable so the
/// scheduler and the tray menu handler can share one source of truth.
#[derive(Debug, Clone)]
pub struct AlertsPausedState {
    inner: Arc<AtomicBool>,
}

impl AlertsPausedState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_paused(&self) -> bool {
        self.inner.load(Ordering::Acquire)
    }

    /// Flip the flag and return the new value.
    pub fn toggle(&self) -> bool {
        // fetch_xor returns the *previous* value; new = !previous.
        let previous = self.inner.fetch_xor(true, Ordering::AcqRel);
        !previous
    }
}

impl Default for AlertsPausedState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_not_paused() {
        let s = AlertsPausedState::new();
        assert!(!s.is_paused());
    }

    #[test]
    fn toggle_flips_and_returns_new_value() {
        let s = AlertsPausedState::new();
        assert!(s.toggle());
        assert!(s.is_paused());
        assert!(!s.toggle());
        assert!(!s.is_paused());
    }

    #[test]
    fn clones_share_state() {
        let a = AlertsPausedState::new();
        let b = a.clone();
        assert!(a.toggle());
        assert!(b.is_paused(), "clone observes the same flag");
    }
}

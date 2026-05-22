//! Platform stop dispatcher.
//!
//! Looks up the session row by id, decides whether a stop is even
//! possible, and shells out to `kill` (Unix) or `taskkill` (Windows).
//! Per build-plan §11.4 the alpha sends a single signal — `SIGTERM` on
//! Unix, `taskkill /F /PID` on Windows. Per-adapter SIGINT-aware
//! cleanup lands when the adapter trait grows a `stop()` method, in a
//! future build step.

use std::sync::Arc;

use agentdeck_adapter::SessionStatus;
use agentdeck_session::get_session;
use agentdeck_storage::Storage;
use agentdeck_telegram::stop::{StopDispatcher, StopOutcome};
use uuid::Uuid;

/// Concrete dispatcher implementation provided by the Tauri shell.
pub struct PlatformStopDispatcher {
    storage: Arc<Storage>,
}

impl PlatformStopDispatcher {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl StopDispatcher for PlatformStopDispatcher {
    fn stop(&self, session_id: Uuid) -> StopOutcome {
        let session = match get_session(&self.storage, session_id) {
            Ok(Some(s)) => s,
            Ok(None) => return StopOutcome::SessionNotFound,
            Err(err) => {
                return StopOutcome::Failed {
                    reason: format!("storage read failed: {err}"),
                };
            }
        };

        if session.status == SessionStatus::Completed {
            return StopOutcome::AlreadyCompleted;
        }

        let Some(pid) = session.pid else {
            return StopOutcome::Unsupported {
                reason: "session has no PID; nothing to terminate".to_string(),
            };
        };

        match platform_kill(pid) {
            Ok(mechanism) => StopOutcome::Success { mechanism },
            Err(reason) => StopOutcome::Failed { reason },
        }
    }
}

#[cfg(unix)]
fn platform_kill(pid: u32) -> Result<String, String> {
    let status = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .map_err(|e| format!("kill failed to spawn: {e}"))?;
    if !status.success() {
        return Err(format!("kill exited with {status}"));
    }
    Ok("SIGTERM".to_string())
}

#[cfg(windows)]
fn platform_kill(pid: u32) -> Result<String, String> {
    let status = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .status()
        .map_err(|e| format!("taskkill failed to spawn: {e}"))?;
    if !status.success() {
        return Err(format!("taskkill exited with {status}"));
    }
    Ok("taskkill".to_string())
}

#[cfg(not(any(unix, windows)))]
fn platform_kill(_pid: u32) -> Result<String, String> {
    Err("no termination mechanism for this OS".to_string())
}

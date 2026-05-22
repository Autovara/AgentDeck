use thiserror::Error;

/// Errors emitted by the process scanner.
///
/// `ProcessSource::list` itself does not fail — backends return an empty
/// list and surface OS errors through `tracing`. `ProcessError` is currently
/// used only by the [`Scanner`](crate::Scanner) supervisor.
#[derive(Debug, Error)]
pub enum ProcessError {
    /// The scanner task already exited and cannot be queried.
    #[error("process scanner is not running")]
    NotRunning,
}

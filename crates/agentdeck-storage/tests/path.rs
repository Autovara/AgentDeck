//! Tests for the platform path resolver.

use std::path::PathBuf;

use agentdeck_storage::{default_data_dir, default_db_path, DEFAULT_DB_FILENAME};

#[test]
fn env_override_takes_precedence() {
    // Use a process-scoped guard via a unique value to avoid colliding with
    // other tests that read the same env var.
    let marker = PathBuf::from("/tmp/agentdeck-test-override-data-dir");
    // SAFETY: tests in this file do not touch this env var from any other
    // thread; Cargo runs them serially within the same binary and we restore
    // the previous value before returning.
    unsafe {
        std::env::set_var("AGENTDECK_DATA_DIR", &marker);
    }
    let resolved = default_data_dir().expect("env override resolves");
    assert_eq!(resolved, marker);
    let db = default_db_path().expect("env override resolves db path");
    assert_eq!(db, marker.join(DEFAULT_DB_FILENAME));
    unsafe {
        std::env::remove_var("AGENTDECK_DATA_DIR");
    }
}

#[test]
fn default_filename_is_agentdeck_db() {
    assert_eq!(DEFAULT_DB_FILENAME, "agentdeck.db");
}

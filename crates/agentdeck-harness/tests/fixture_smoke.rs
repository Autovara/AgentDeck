use std::path::Path;

use agentdeck_core::FileEventKind;
use agentdeck_harness::Fixture;

#[test]
fn loads_example_aider_presence_fixture() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("sessions")
        .join("example-aider-presence")
        .join("fixture.json");
    let fixture = Fixture::load(&path).expect("fixture should load");

    assert_eq!(fixture.name, "example-aider-presence");
    assert_eq!(fixture.agent, "aider");
    assert_eq!(fixture.agent_version, "0.55.0");
    assert_eq!(fixture.capture_os, "linux-x86_64");
    assert_eq!(fixture.duration_ms, 10_000);

    assert_eq!(fixture.processes.len(), 1);
    let proc = &fixture.processes[0];
    assert_eq!(proc.pid, 12345);
    assert_eq!(proc.name, "aider");
    assert_eq!(proc.cwd.as_deref(), Some("<REPO>/billing-api"));

    assert_eq!(fixture.files.len(), 1);
    let file = &fixture.files[0];
    assert_eq!(file.path, "<REPO>/billing-api/.aider.chat.history.md");
    assert_eq!(file.events.len(), 1);
    assert_eq!(file.events[0].kind, FileEventKind::Created);
    assert_eq!(file.events[0].at_ms, 100);
    assert_eq!(file.events[0].content_ref.as_deref(), Some("history-0.md"));

    let expectations = fixture.expectations.as_ref().expect("expectations present");
    assert_eq!(expectations.final_status.as_deref(), Some("running"));
    assert!(expectations
        .must_emit_events
        .contains(&"session_detected".to_string()));
}

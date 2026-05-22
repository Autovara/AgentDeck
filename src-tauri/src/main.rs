// Prevent the extra Windows console window in release builds. On debug we keep
// it so that `tracing` output is visible during development.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    agentdeck_app::run();
}

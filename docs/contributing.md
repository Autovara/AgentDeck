# Contributing

> Status: draft. Updated before the repo goes public.

This page will describe:

- how to set up a development environment on macOS, Windows, and Linux
- how to run the dev-time agent harness so you do not need real AI agents to develop adapters
- how to add a new adapter and declare its capability level
- how to write fixture-based tests
- how to propose changes to the alpha roadmap

## Development environment

AgentDeck uses a Cargo workspace. The first crates live under `crates/`:

- `agentdeck-core` — OS-facing trait definitions (`ProcessSource`, `FileWatcher`, `Clock`) and core types
- `agentdeck-harness` — dev-time replay harness for adapter testing without real AI agents

### Prerequisites

- Rust stable (install via [rustup](https://rustup.rs))
- A C linker on your platform:
  - Linux: `sudo apt install build-essential` (Debian/Ubuntu) or distro equivalent
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Windows: the Visual Studio Build Tools

### Common commands

- `cargo check --workspace` — type-check every crate
- `cargo test --workspace` — run unit and integration tests
- `cargo run --bin agentdeck-harness -- --help` — see harness subcommands

The Tauri app skeleton lands in a later step. Fixture recordings live under `fixtures/sessions/`.

## Code of conduct

To be filled before the repo goes public.

## Issue and pull request templates

To be filled before the repo goes public.

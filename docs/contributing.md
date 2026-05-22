# Contributing

AgentDeck is a Tauri 2.x desktop app with a React + TypeScript frontend and a Rust backend organised as a Cargo workspace. The lead validation platform is **Linux**; the dev loop also runs on macOS and Windows, and CI keeps both cross-compiling alongside Linux.

This guide covers the dev loop, the workspace shape, and the conventions to follow when changing things. The per-crate design rationale lives in each crate's `lib.rs` documentation — `cargo doc --open` is the source of truth for internals.

## Development environment

### Prerequisites

- Rust **stable** (install via [rustup](https://rustup.rs)).
- Node.js **20+** and **pnpm 11** (`packageManager` is pinned in `package.json`).
- A C linker:
  - **Linux:** `sudo apt install build-essential` (or distro equivalent)
  - **macOS:** `xcode-select --install`
  - **Windows:** Visual Studio Build Tools with the C++ workload
- **Linux only** — the Tauri 2.x system libraries:
  ```sh
  sudo apt install -y \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
      librsvg2-dev libsoup-3.0-dev libxdo-dev libssl-dev \
      build-essential pkg-config curl wget file patchelf
  ```
  Other distros: install the equivalent WebKitGTK 4.1, GTK 3, libsoup 3, AyatanaAppIndicator 3, and librsvg development packages.

### Common commands

```sh
# Install Node deps
pnpm install --frozen-lockfile

# Hot-reloading dev loop (Vite on :1420 + Tauri webview)
pnpm tauri:dev

# Release bundle for the host OS (.deb + .AppImage on Linux)
pnpm tauri:build

# Rust gates
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

# Frontend gates
pnpm typecheck
pnpm build
```

CI runs the same gates with `--locked` / `--frozen-lockfile`.

### Logging

`AGENTDECK_LOG` overrides the default `info` filter for the Tauri shell:

```sh
AGENTDECK_LOG=debug pnpm tauri:dev
AGENTDECK_LOG=agentdeck_telegram=trace,info pnpm tauri:dev
```

Syntax follows `tracing-subscriber`'s `EnvFilter`.

### Pointing the dev build at a throwaway database

`AGENTDECK_DATA_DIR` overrides the parent directory for the SQLite database, useful so dev runs don't touch your real one:

```sh
AGENTDECK_DATA_DIR=/tmp/agentdeck-dev pnpm tauri:dev
```

The default paths are:

- macOS: `~/Library/Application Support/AgentDeck/agentdeck.db`
- Windows: `%APPDATA%\AgentDeck\agentdeck.db`
- Linux: `$XDG_DATA_HOME/agentdeck/agentdeck.db` (defaults to `~/.local/share/agentdeck/agentdeck.db`)

## Workspace layout

Sixteen internal crates plus the Tauri shell and the React frontend. Each crate's `lib.rs` documents its own contract; this list is a navigation aid.

| Crate | Owns |
|---|---|
| `agentdeck-core` | OS trait definitions (`ProcessSource`, `FileWatcher`, `Clock`) |
| `agentdeck-harness` | Dev-time replay harness library + CLI; fixture format + redactor |
| `agentdeck-process` | `SysinfoProcessSource`, `ProcessScanner`, snapshot diffing |
| `agentdeck-storage` | SQLite schema, migrations, the `Storage` handle |
| `agentdeck-adapter` | `Adapter` trait, `AdapterRegistry`, per-scan result + diagnostic shapes |
| `agentdeck-adapter-aider` | Built-in Level 1 adapter for [Aider](https://aider.chat) |
| `agentdeck-adapter-codex` | Built-in Level 1 adapter for the [Codex CLI](https://github.com/openai/codex) |
| `agentdeck-adapter-claude-code` | Built-in Level 1 adapter for [Claude Code](https://github.com/anthropics/claude-code) |
| `agentdeck-adapter-custom` | User-defined Level 1 regex matchers + CRUD |
| `agentdeck-session` | `SessionStateMachine`, `Session` model, `sessions` + `session_events` |
| `agentdeck-attention` | `AttentionEngine`, attention rules, `attention_items` |
| `agentdeck-diagnostics` | `record` / `list` over `adapter_diagnostics` |
| `agentdeck-tags` | `project_tags` CRUD + per-session tag assignment |
| `agentdeck-export` | CSV (RFC-4180) + JSON serialisation of session rows |
| `agentdeck-telegram` | Bot pairing, allowlist, commands, rate limiter, `/stop` confirmation, audit + `remote_commands` |
| `src-tauri/` | Tauri shell — tray probe, monitor tick orchestrator, platform stop dispatcher, notifications, every Tauri command consumed by the dashboard |
| `src/` | React 19 + TypeScript dashboard rendered inside the Tauri webview |

The Tauri shell deliberately stays thin — it wires the domain crates together; product logic lives in the crates.

## Dashboard surfaces

Five pages, navigable from the left sidebar:

| Page | Tauri commands | Notes |
|---|---|---|
| Overview | `get_overview_report` | Summary tiles + 5 recent attention items. |
| Attention | `get_attention_report`, `mute_attention_item`, `resolve_attention_item` | Full open-items list with severity / reason filters. |
| Sessions | `list_sessions`, `assign_session_tag`, `clear_session_tag` | Every session (active + completed) with inline tag dropdown and cost label. |
| Diagnostics | `get_tray_surface`, `get_storage_report`, `get_process_scanner_report`, `get_custom_adapter_report`, `get_adapter_diagnostics`, `add_custom_adapter`, `delete_custom_adapter`, `set_custom_adapter_enabled` | Read surface + custom adapter CRUD. |
| Settings | `list_project_tags`, `create_project_tag`, `delete_project_tag`, `export_sessions_csv`, `export_sessions_json`, every `*_telegram*` command | Project tags, export, Telegram pairing. |

The "Refresh now" button drives `run_monitor_tick` and then refreshes Overview, Attention, Sessions, the process scanner, and adapter diagnostics in parallel. A 15 s background scheduler (`src-tauri/src/tray_scheduler.rs`) drives the same tick, updates the tray menu, and sends desktop notifications for newly-created urgent attention items.

## Conventions

### Tests

- Every crate ships unit and / or integration tests against an in-memory SQLite database (`Storage::open_in_memory()`).
- Integration tests live under `crates/*/tests/` and use the same in-memory pattern.
- The whole workspace runs in well under a second: `cargo test --workspace --locked` is part of the CI gate.

When adding storage writes, assert the SQL `CHECK` constraints are honoured — the schema has tight enums for `status`, `severity`, `cost_kind`, etc., and a wrong value yields an opaque rusqlite error in production.

### Schema migrations

Migrations are **immutable once shipped**. Add a new file `crates/agentdeck-storage/migrations/NNNN_short_name.sql`, register it in `crates/agentdeck-storage/src/schema.rs`, and write a regression test under `crates/agentdeck-storage/tests/migrate.rs`. Re-opening a database that already has a higher version is a no-op.

### Adding a built-in adapter

1. New crate `agentdeck-adapter-<name>` modelled on `agentdeck-adapter-codex` (single-file `lib.rs`, ~450 LOC including tests).
2. `impl Adapter for FooAdapter`: stable lower-case `name()`, fixed `capability_level()`, pure `scan()` producing one `AdapterMatch` per detected process plus a fresh `AdapterDiagnostic`.
3. Register the adapter in `src-tauri/src/monitor_tick.rs` (`registry.register(Box::new(FooAdapter::new()))`). Built-in adapters always run before the custom adapter so their rows appear first in the diagnostics card.
4. Cover the matcher's contract with unit tests: positive matches across binary-name, absolute-path cmdline, and interpreter-launched variants; negative matches for lookalikes that must not trigger.

Custom adapters need no code change — they're configured at runtime from the Diagnostics page.

### Capability levels and the attention engine

The attention engine **gates** `waiting_for_input` items to adapters at `CapabilityLevel::Status` or higher. Level 1 adapters never produce a waiting-for-input attention item, even if `AdapterMatch.status` happens to claim that — the gate is in `agentdeck-attention::rules`. When promoting an adapter to Level 2, the gate flips automatically; no engine change required. See [`adapter-capabilities.md`](adapter-capabilities.md) for the full level matrix.

### Cost data

Each `AdapterMatch` carries `cost_per_hour_cents: Option<i64>`. When `Some`, the session state machine writes `estimated_cost = rate × elapsed / 3600 / 100` on every tick and labels it `cost_kind = estimated`. Built-in adapters declare `None` in the alpha; only the custom adapter currently populates a real rate (from its `cost_per_hour_cents` column).

## Code of conduct

To be filled before the repo goes public. For now, please report concerns via GitHub issues.

## Issue and pull request templates

To be filled before the repo goes public.

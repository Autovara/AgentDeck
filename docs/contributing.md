# Contributing

> Status: draft. Updated before the repo goes public.

This page will describe:

- how to set up a development environment on macOS, Windows, and Linux
- how to run the dev-time agent harness so you do not need real AI agents to develop adapters
- how to add a new adapter and declare its capability level
- how to write fixture-based tests
- how to propose changes to the alpha roadmap

## Development environment

AgentDeck is a Tauri 2.x desktop app with a React + TypeScript frontend and a Rust backend organised as a Cargo workspace.

Workspace layout:

- `crates/agentdeck-core` — OS-facing trait definitions (`ProcessSource`, `FileWatcher`, `Clock`) and core types
- `crates/agentdeck-harness` — dev-time replay harness library and CLI
- `crates/agentdeck-process` — `SysinfoProcessSource` and the `ProcessScanner` (snapshot + diff over any `ProcessSource`)
- `crates/agentdeck-storage` — SQLite schema, migrations, and the `Storage` handle the monitor core uses
- `crates/agentdeck-adapter` — `Adapter` trait, `AdapterRegistry`, and the per-scan result + diagnostic shapes adapters produce
- `crates/agentdeck-adapter-aider` — built-in Level 1 adapter for the [Aider](https://aider.chat) CLI (presence-only)
- `crates/agentdeck-adapter-custom` — user-defined Level 1 process matcher (definitions, validation, persistence, runtime adapter)
- `crates/agentdeck-session` — `SessionStateMachine` and `Session` / `SessionEvent` repository over `sessions` and `session_events`
- `crates/agentdeck-attention` — rule-based attention engine over `attention_items`, plus `AttentionEngine::apply` / `set_mute` / `resolve`
- `crates/agentdeck-diagnostics` — `record` / `list` over the `adapter_diagnostics` table; persists one row per `adapter_name` per tick (no history)
- `src-tauri/` — Tauri application shell, tray detection, storage bootstrap, process scanner wiring, custom-adapter wiring, the monitor-tick orchestrator, and the Tauri commands consumed by the dashboard
- `src/` — React 19 + TypeScript dashboard rendered inside the Tauri webview
- `fixtures/sessions/` — recorded harness fixtures

### Prerequisites

- Rust stable (install via [rustup](https://rustup.rs))
- Node.js 20 or newer and [pnpm](https://pnpm.io/) (the repo pins pnpm via `packageManager`)
- A C linker on your platform:
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Windows: the Visual Studio Build Tools (with the C++ workload)
  - Linux: `sudo apt install build-essential` (Debian/Ubuntu) or distro equivalent
- On Linux, the Tauri 2.x system libraries:
  ```bash
  sudo apt install \
    libwebkit2gtk-4.1-dev libssl-dev libayatana-appindicator3-dev \
    librsvg2-dev libxdo-dev libsoup-3.0-dev libgtk-3-dev \
    patchelf file pkg-config
  ```
  Other distributions: install the equivalent WebKitGTK 4.1, GTK 3, libsoup 3, AyatanaAppIndicator 3, and librsvg development packages.

### Common commands

Rust:

- `cargo check --workspace` — type-check every crate
- `cargo test --workspace` — run unit and integration tests
- `cargo clippy --workspace --all-targets -- -D warnings` — lint
- `cargo fmt --all` — format
- `cargo run --bin agentdeck-harness -- --help` — see harness subcommands

Frontend:

- `pnpm install` — install Node dependencies
- `pnpm typecheck` — run `tsc` against both the app and the Vite config
- `pnpm build` — type-check and produce a production bundle in `dist/`

Tauri app:

- `pnpm tauri:dev` — start the dev loop. Spawns Vite on `http://localhost:1420` and launches the Tauri webview against it. Rust changes trigger a rebuild and relaunch; React changes hot-reload in place.
- `pnpm tauri:build` — produce a release bundle for the host OS. Bundling targets are configured in `src-tauri/tauri.conf.json`.

### Logging

Set `AGENTDECK_LOG` to override the default `info` log level for the Tauri shell, e.g.:

```bash
AGENTDECK_LOG=debug pnpm tauri:dev
```

The format and target filters follow the `tracing-subscriber` `EnvFilter` syntax.

### Tray surface and the dashboard fallback

At startup the Rust shell probes the host environment to decide whether the tray icon will be the primary surface. The result is exposed to the dashboard via the `get_tray_surface` Tauri command and rendered in the "Tray surface" card on the Overview page. When no tray is available (notably stock GNOME without the AppIndicator extension, and some Wayland sessions), AgentDeck opens the dashboard window directly. See `src-tauri/src/tray.rs` for the probe and the per-desktop decision table.

### Local SQLite storage

AgentDeck stores all session, attention, usage, audit, and settings data in a single SQLite database. The path is platform-specific:

- macOS: `~/Library/Application Support/AgentDeck/agentdeck.db`
- Windows: `%APPDATA%\AgentDeck\agentdeck.db`
- Linux: `$XDG_DATA_HOME/agentdeck/agentdeck.db` (defaults to `~/.local/share/agentdeck/agentdeck.db`)

Set `AGENTDECK_DATA_DIR` to override the parent directory; this is also how the smoke tests and CI runs keep the real user database untouched, e.g.:

```bash
AGENTDECK_DATA_DIR=/tmp/agentdeck-dev pnpm tauri:dev
```

The shell opens the database during `setup()`, applies all migrations defined in `crates/agentdeck-storage/migrations/`, and configures `journal_mode=WAL`, `synchronous=NORMAL`, and `foreign_keys=ON`. Migrations are immutable once shipped; add new schema changes as a new `NNNN_*.sql` file and register it in `crates/agentdeck-storage/src/schema.rs`.

To reset a development database, delete the file (and the `.db-wal`, `.db-shm` siblings) at the path above. To inspect it, use any modern SQLite tool — the schema is tagged `STRICT` so the table definitions are self-documenting.

### Process scanner

`crates/agentdeck-process` exposes a `ProcessScanner` that wraps any `ProcessSource` (from `agentdeck-core`) and produces wall-clock-stamped snapshots, plus a diff between consecutive snapshots. Production builds use `SysinfoProcessSource`, backed by the [`sysinfo`](https://crates.io/crates/sysinfo) crate with a minimal feature set (`default-features = false, features = ["system"]`). Tests substitute `agentdeck-harness::MockProcessSource` through the same trait.

The Tauri shell currently invokes the scanner on demand from the `get_process_scanner_report` command (rendered as the "Process scanner" card on the dashboard). Once the monitor core lands, that command will read a cached snapshot maintained by a background tick loop instead of calling sysinfo synchronously.

The scanner also exposes a lightweight `extract_candidates` helper that surfaces processes whose name or command line contains an alpha agent pattern (`aider`, `codex`, `claude`, `ollama`, `agent`). This is **not** the adapter framework — it exists so the diagnostics card can preview what adapters might see. Adapters will replace it with richer matching (exe path, parent process, file watchers, version probes).

### Custom adapters

`crates/agentdeck-adapter-custom` is the first concrete adapter. It persists user-defined matchers in the `custom_adapters` SQLite table (see migration `0002_custom_adapters.sql`) and, given a `ProcessSnapshot`, returns every process that matches each enabled adapter.

Each definition has a label, an agent name, an enabled flag, an optional colour, an optional notes field, an optional `cost_per_hour_cents` for coarse cost reporting, and one of three match kinds:

- `name` — Rust regex against the process name.
- `cmdline` — Rust regex against the joined command line (single-space separator).
- `cwd` — Rust regex against the process's current working directory. Processes without a `cwd` never match.

Custom adapters are deliberately locked to capability **Level 1** (presence detection only). Status, usage, and `/stop` are out of scope; the higher capability levels belong to the native adapters that follow in later build-plan steps.

The "Custom adapters" card on the dashboard exposes list / add / delete / enable / disable. Add and delete flow through the `add_custom_adapter`, `delete_custom_adapter`, and `set_custom_adapter_enabled` Tauri commands; all four also re-run the matcher so the card shows current PID matches without a second round trip.

To delete every definition during development, drop the database (see "Local SQLite storage" above) or open it with any SQLite tool and `DELETE FROM custom_adapters;`.

### Session state machine

`crates/agentdeck-session` owns the `sessions` and `session_events` tables. The state machine consumes the `AdapterMatch` set produced by the registry on every tick and decides, for each `(adapter_name, pid)` pair, whether it creates a new session row or refreshes an existing one. A session is uniquely identified at runtime by `(adapter_name, pid)` among non-completed rows; PIDs of newly-completed sessions are eligible for reuse on the next tick. Any active session not seen in the current match set is marked `completed` immediately (no grace tick in the alpha). The whole apply runs in one SQLite transaction, so an interrupted tick never leaves the database half-written.

### Attention engine

`crates/agentdeck-attention` reads `sessions` (and the `TickReport` from the session state machine) and writes `attention_items`. The rule set is deliberately small in the alpha:

- `rate_limited` → `rate_limit` (severity `warn`)
- `errored` → `command_failed` (severity `warn`)
- `stalled` → `stalled_session` (severity `info`)
- `waiting_for_input` → `waiting_for_input` (severity `info`), **gated to adapter level ≥ `Status`** so Level 1 (presence-only) custom adapters never produce a waiting attention item — see build-plan §10 TUI caveat.

`AttentionEngine::apply(&TickReport, &[Session])` is idempotent: re-running with the same input leaves the row counts unchanged. Severity / message / source / confidence / recommended actions update in place; `created_at` is preserved across updates because the item's age is the moment it first started worrying. When the session completes, or the rule stops firing, the row is resolved (`resolved_at` set). When the user mutes an item (`set_mute(id, Some(until))`), the engine skips updates for that row until the mute expires; the resolve path is unaffected.

The monitor-tick orchestrator lives in `src-tauri/src/monitor_tick.rs`. It runs one tick on demand via the `run_monitor_tick` Tauri command: snapshot → registry → `SessionStateMachine::apply` → `AttentionEngine::apply`. The result includes IDs created/updated/completed for sessions and created/updated/resolved/skipped-muted for attention items. The background scheduler that ticks this on a steady cadence lands in a later build step.

The Tauri commands behind the attention surface are `get_attention_report`, `mute_attention_item`, and `resolve_attention_item`.

### Aider adapter

`crates/agentdeck-adapter-aider` ships the alpha's first built-in Level 1 adapter for [Aider](https://aider.chat). The matcher is **process-only** in the alpha: a process is treated as an Aider session when (1) its OS-reported name is `aider`/`aider.exe`, (2) its `cmdline[0]` basename is `aider`, or (3) the process is a `python*` interpreter and `cmdline` contains either `-m aider[.<sub>]` or a script path whose basename is `aider`. False positives are possible if the user has an unrelated script literally named `aider`; the simpler signal is preferred for the alpha because the user can disable the adapter via the dashboard if needed.

The adapter is locked to `CapabilityLevel::Presence` per build-plan §10's TUI caveat: Aider is an interactive TUI and we do not yet have a defensible waiting-detection strategy, so the adapter must not surface `waiting_for_input` attention items. The attention engine's existing Level >= `Status` gate enforces this automatically — `AiderAdapter` is registered in `MonitorTickState::run_tick` between the snapshot and the session state machine apply, just like the custom adapter, and its diagnostics flow through `agentdeck-diagnostics::record`.

The unit tests under `crates/agentdeck-adapter-aider/src/lib.rs` cover the matcher (positive cases: bare `aider`, `aider.exe`, absolute-path `cmdline[0]`, `python -m aider[.cli]`, `python3.11 -m aider`, Windows `python.exe C:\\...\\aider`; negative cases: unrelated python and node processes, `pythonista` / `aiderbot` lookalikes, `python -m venv`, pathological `-m` with no following argument) and the `Adapter` trait surface (stable name and level, empty / single / multiple matches, and the diagnostic's `last_scan_time` linking back to the snapshot).

### Adapter diagnostics

`crates/agentdeck-diagnostics` owns the `adapter_diagnostics` table. The schema (defined in `agentdeck-storage/migrations/0001_initial.sql`) holds at most one row per `adapter_name` — the §17 "keep only the most recent scan per adapter; do not accumulate history" discipline is enforced at the SQL level with `INSERT OR REPLACE` on the `adapter_name` primary key.

`record(&Arc<Storage>, &[AdapterDiagnostic])` runs the whole batch in a single transaction. Adapters absent from the current batch are *not* deleted — their previously recorded row stays so the user can still see the most recent diagnostic for an adapter that did not run this tick. `list(&Arc<Storage>)` returns every row, ordered by `last_scan_time ASC` so stale adapters surface at the top of the dashboard card.

The orchestrator (`src-tauri/src/monitor_tick.rs`) calls `record` immediately after `AdapterRegistry::scan_all` and before the session state machine apply. Persistence is **best-effort**: a failure is logged via `tracing::warn!` but the rest of the tick continues, because an empty diagnostics card is recoverable while a missed session update is not.

The Tauri command behind the card is `get_adapter_diagnostics`; the React `AdapterDiagnosticsCard` renders one table row per adapter with an expandable per-row `<details>` block for data sources, missing permissions, failure reasons, and known limitations. The Diagnostics sidebar item carries a warning badge with the count of adapters that have non-empty `failure_reasons`.

### Dashboard pages

The dashboard is split into three pages, navigable from the left sidebar:

- **Overview** — four summary tiles (active agents, open attention by severity, stalled / waiting, estimated cost today) plus a five-item "recent attention" strip. The page is driven by one Tauri call, `get_overview_report`, which composes the snapshot from `agentdeck-session::list_active` and `AttentionEngine::list_open`. The "estimated cost today" tile shows `—` until cost tracking lands (build-plan §15 step 18).
- **Attention** — the full open-items list, with severity (urgent / warn / info), reason (every enum value present in the current dataset), and "show muted" filter chips. Filtering is client-side over the same `get_attention_report` payload the Overview strip uses, so the lists never disagree.
- **Diagnostics** — the existing tray-surface, storage, process-scanner, and custom-adapter cards, plus the **Adapter diagnostics** card driven by `get_adapter_diagnostics`.

The "Refresh now" button in the sidebar footer drives `run_monitor_tick` and then refreshes Overview, Attention, and the process scanner in parallel. The button's last-tick timestamp gives users a rough sense of how stale the page is. A scheduled background tick lands in a later build step; until then every dashboard refresh is user-initiated.

## Code of conduct

To be filled before the repo goes public.

## Issue and pull request templates

To be filled before the repo goes public.

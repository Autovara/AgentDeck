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
- `crates/agentdeck-storage` — SQLite schema, migrations, and the `Storage` handle the monitor core uses
- `src-tauri/` — Tauri application shell, tray detection, storage bootstrap, and Tauri commands consumed by the dashboard
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

## Code of conduct

To be filled before the repo goes public.

## Issue and pull request templates

To be filled before the repo goes public.

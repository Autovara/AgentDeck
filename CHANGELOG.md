# Changelog

All notable changes to AgentDeck are recorded in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/). This project follows [SemVer](https://semver.org/); pre-`1.0` versions communicate alpha / beta status through pre-release suffixes.

## [0.1.0-alpha.1] — 2026-05-22

First packaged alpha. Lead validation platform: Linux. macOS and Windows builds arrive at beta.

### Added

- **Local monitoring core.** SQLite-backed `Storage` with versioned migrations; `agentdeck-process` snapshot scanner over `sysinfo`; the `Adapter` trait + `AdapterRegistry` shared by every concrete adapter; `SessionStateMachine` writing `sessions` + `session_events`; rule-based `AttentionEngine` writing `attention_items`.
- **Built-in adapters at Level 1 (Presence):**
  - **Aider** — recognises bare `aider`, `aider.exe`, `python -m aider[.<sub>]`, venv entrypoints.
  - **Codex CLI** — recognises `codex` (npm shim and bin), `node /path/codex`, `node --enable-source-maps /opt/codex/bin/codex`.
  - **Claude Code** — recognises `claude` and the older `claude-code`, plus node-interpreter variants.
- **Custom process adapter.** User-defined regex matchers against process name, joined command line, or `cwd`. CRUD UI on the Diagnostics page. Carries an optional `cost_per_hour_cents` field that feeds session cost estimation.
- **Per-tick orchestration.** `monitor_tick.rs` wires: process snapshot → adapter registry → session state machine → attention engine, with adapter diagnostics persisted best-effort via `agentdeck-diagnostics`.
- **Cost tracking (estimated).** Adapter-declared `cost_per_hour_cents` flows through `AdapterMatch` into `sessions.estimated_cost`. The state machine writes it on every tick with `cost_kind = estimated`. Built-in adapters declare `None`; only the custom adapter populates this in the alpha.
- **Project / client tags.** Full CRUD on `project_tags` plus per-session assignment from the Sessions page. Deleting a tag clears it from every session that referenced it inside one transaction.
- **Sessions export.** CSV (hand-rolled RFC-4180) and JSON of every session row, via `tauri-plugin-dialog`'s save dialog. Buttons live on the Settings page.
- **Dashboard.** Five pages (Overview, Attention, Sessions, Diagnostics, Settings) backed by typed Tauri commands. Sidebar shows badges for unresolved attention severity, failing adapters, tray fallback, and Telegram state.
- **Tray + notifications.** Per-OS tray probe with dashboard fallback. Live menu rebuilt every 15s from a fresh tick: `N active · M waiting`, top-5 attention rows with severity prefix, Refresh, Pause Alerts, Quit. Desktop notifications fire for newly-created urgent attention items (suppressed when alerts are paused).
- **Telegram.** Bot pairing via single-use 6-character codes, allowlist of paired user ids, read-only commands (`/help`, `/status`, `/agents`, `/attention`, `/session <id>`), `/mute <id> [hours]` with audit, `/stop <id>` with mandatory `STOP <id>` confirmation within 60s and platform-default termination (`SIGTERM` / `taskkill /F /PID`). Per-user rate limit (30/min, burst 10). Bot only contacts `api.telegram.org`.
- **Packaging.** `.deb` and `.AppImage` artifacts for Linux via `pnpm tauri build`. Declared `.deb` runtime depends on `libwebkit2gtk-4.1-0`, `libgtk-3-0`, `libayatana-appindicator3-1`.
- **CI.** GitHub Actions workflow runs `cargo build / clippy -D warnings / test --workspace --locked` plus `pnpm typecheck` on Ubuntu against every push and PR.
- **Documentation.** README (with known-limitations and reporting-issues sections), install guide, adapter capabilities, Telegram setup, privacy, security, contributing.

### Security

- Bot token stored as plain text in the `settings` table (alpha). Documented in [`docs/security.md`](docs/security.md); the OS-keychain path lands at beta.
- Every Telegram-driven state change (enable/disable, token set/cleared, pair, revoke, attention mute, `/stop` request, `/stop` execute) writes a row to `audit_log`. The token itself is never logged.

### Known limitations

See the **Known limitations** section in [`README.md`](README.md).

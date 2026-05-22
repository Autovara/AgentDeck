# Alpha Limitations

> Status: alpha. Updated as the alpha is built; current build: **0.1.0-alpha.1**.

This document tracks what does and does not work in the current alpha build. It is the source of truth for the gap between the current build and the V1 product.

## Lead platform and parity

The alpha is validated against a single lead platform with the other platforms continuing to build in CI. Full macOS, Windows, and Linux parity is required by beta.

- **Lead platform:** Linux (Ubuntu 24.04 LTS, Fedora 41+, KDE Plasma 6). Daily-driver validation target during the alpha.
- **macOS parity:** deferred to beta. The Tauri shell compiles, but no macOS bundle is produced in the alpha.
- **Windows parity:** deferred to beta. The Tauri shell compiles, but no Windows bundle is produced in the alpha.
- **Linux distro coverage outside the primary targets:** best-effort. Users on stock GNOME may need the AppIndicator extension to see a tray icon; the dashboard-plus-notifications fallback works without it.

## Packaging

- Two artifacts per alpha build: `.deb` (Debian / Ubuntu) and `.AppImage` (portable). See [`install.md`](install.md).
- **Binaries are unsigned.** Linux has no Gatekeeper-equivalent prompt, but verify the SHA-256 of the bundle against the release page checksum before installing. Code signing + notarisation arrive with the first macOS / Windows builds at beta.
- **No auto-update.** Alpha users update by reinstalling. The auto-update channel is build-plan §17 work for beta.

## Adapters

See [`adapter-capabilities.md`](adapter-capabilities.md) for the live capability matrix.

| Agent | Detection | Status | Usage | Control | Notes |
|---|---|---|---|---|---|
| Claude Code | ✅ Level 1 (process) | ❌ Level 2 (TUI caveat) | ❌ Level 3 | ❌ Level 4 | Detects `claude` / `claude-code` and node-launched variants |
| Codex CLI | ✅ Level 1 (process) | ❌ Level 2 (TUI caveat) | ❌ Level 3 | ❌ Level 4 | Detects `codex` and node-launched variants |
| Aider | ✅ Level 1 (process) | ❌ Level 2 (TUI caveat) | ❌ Level 3 | ❌ Level 4 | Detects `aider`, `python -m aider`, venv entrypoints |
| Custom process | ✅ Level 1 (regex) | ❌ N/A | ❌ N/A | ❌ N/A | User-defined match against name / cmdline / cwd |

Level 2 status classification (waiting / stalled / errored) on Aider's chat-history mtime is the first beta target.

## Telegram

- ✅ Pairing with single-use code + allowlist.
- ✅ Read-only commands: `/help`, `/status`, `/agents`, `/attention`, `/session <id>`.
- ✅ `/mute <id> [hours]` with audit log.
- ✅ `/stop <id>` with `STOP <id>` confirmation and platform-default termination (`SIGTERM` / `taskkill`).
- ❌ Per-adapter stop overrides (SIGINT-aware TUI cleanup, Win32 console control) — beta.
- ❌ Telegram push when AgentDeck creates an urgent attention item — later.
- ❌ Bot-token encryption at rest (currently plaintext in the local DB) — beta, via OS keychain. See [`security.md`](security.md).

## Tray and dashboard

- ✅ Tray availability probe + dashboard fallback. The Diagnostics page shows what was detected.
- ✅ Tray menu lists active session count, attention items, Refresh, Pause Alerts, Quit.
- ✅ Desktop notifications for urgent attention items, gated by the Pause Alerts toggle.
- ❌ Tray cost line shows `$— today` (cost summary lands later; see Usage below).
- ❌ Background tick scheduler runs at 15s on a fixed interval; configurable cadence is later.

## Cost / Usage / Projects

- ✅ Per-session `estimated_cost` is computed when an adapter declares `cost_per_hour_cents`. Today only the custom adapter does (user-configured); built-in adapters declare `None` and so show `—`.
- ✅ Project tags: CRUD + per-session assignment from the Sessions page.
- ✅ Sessions export (CSV + JSON) from the Settings page.
- ❌ Daily / weekly cost summaries — no Usage page in alpha.
- ❌ Budget threshold alerts — not yet implemented.
- ❌ Auto-tagging by `cwd` / `repo_path` regex — not yet implemented.
- ❌ Multi-currency — USD only.

## Known issues

- The cost line on the tray menu always reads `$— today`; the Overview page is the source of truth until the tray scheduler reads the same number.
- Sessions in `completed` status remain visible on the Sessions page (intentional for cost history) but are not displayed on the Overview attention list.
- If you delete a project tag while sessions reference it, those sessions show "(deleted)" in the dropdown for one render before the tag list refreshes.

## Reporting alpha issues

Open an issue on GitHub. Useful information:

- AgentDeck build version (see app menu or the `agentdeck --version` output).
- Linux distro + desktop environment (`echo "$XDG_CURRENT_DESKTOP / $XDG_SESSION_TYPE"`).
- The Diagnostics page screenshot if the issue is detection-related.
- The relevant slice of `~/.local/share/agentdeck/agentdeck.db` is fair to share *only if* you have not paired Telegram; the bot token is plaintext at rest in the alpha.

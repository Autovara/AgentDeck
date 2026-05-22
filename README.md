# AgentDeck

> Local-first attention board and safe Telegram command center for AI coding agents.

AgentDeck tracks the AI coding assistants running on your machine — Claude Code, Codex CLI, Aider, plus any process you teach it to recognise — and tells you which sessions need a human. It runs without an account, stores everything in a local SQLite database, and exposes a small Telegram surface for status checks and a confirmation-gated `/stop`.

## Status

**Alpha — `0.1.0-alpha.1`.** Linux is the lead validation platform. macOS and Windows builds arrive at beta. See [`CHANGELOG.md`](CHANGELOG.md) for what shipped.

## Features

| Surface | Capabilities |
|---|---|
| **Detection** | Built-in Level 1 adapters for Aider, Codex CLI, Claude Code. Custom regex matchers (name / cmdline / cwd) defined from the dashboard. |
| **Dashboard** | Five pages — Overview, Attention, Sessions, Diagnostics, Settings. Auto-refreshes on a 15 s tick. |
| **Tray** | Live menu with active-session counts, top attention items, Pause Alerts toggle. Falls back to dashboard + desktop notifications when no tray surface is available. |
| **Cost / tags / export** | Per-session estimated cost from adapter-declared rates. Project / client tags, assignable per session. CSV + JSON export of every session row. |
| **Telegram** | Pairing via one-time code, allowlisted user IDs, read-only commands (`/status`, `/agents`, `/attention`, `/session`), `/mute`, and `/stop` with mandatory `STOP <id>` confirmation. Per-user rate limit. Every action audited. |

## Install

End users — see [`docs/install.md`](docs/install.md) for prerequisites, supported distros, and AppImage caveats. The short version:

```sh
# Debian / Ubuntu
sudo apt install ./agentdeck_0.1.0-alpha.1_amd64.deb

# Anywhere else on Linux
chmod +x ./agentdeck_0.1.0-alpha.1_amd64.AppImage
./agentdeck_0.1.0-alpha.1_amd64.AppImage
```

The app creates its database at `~/.local/share/agentdeck/agentdeck.db` on first run. Delete that file to start fresh.

## Build from source

Contributors — see [`docs/contributing.md`](docs/contributing.md) for the full dev loop. The short version on Ubuntu 24.04 or similar:

```sh
sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev build-essential \
    pkg-config curl wget file
curl https://sh.rustup.rs -sSf | sh -s -- -y
npm i -g pnpm@11

pnpm install --frozen-lockfile
pnpm tauri:dev     # hot-reloading dev loop
pnpm tauri:build   # produces .deb + .AppImage under src-tauri/target/release/bundle/
```

CI runs `cargo build / clippy -D warnings / test --workspace --locked` and `pnpm typecheck` on every push.

## Privacy

- Core monitoring runs **locally only**. AgentDeck has no telemetry and never phones home.
- When Telegram is enabled, AgentDeck contacts **only `api.telegram.org`** over HTTPS. The bot token is stored locally; pairing is gated by a one-time code; only allowlisted Telegram user IDs receive any reply beyond the pair prompt.
- The sessions export writes to a path you pick via the OS save dialog — nothing is uploaded.

See [`docs/privacy.md`](docs/privacy.md) and [`docs/security.md`](docs/security.md) for the threat model and storage details.

## Known limitations

- **Linux only in the alpha.** macOS and Windows compile but do not ship a bundle yet.
- **Alpha binaries are unsigned.** Linux has no Gatekeeper-equivalent prompt; verify the SHA-256 of the artifact against the release page before installing. Code signing + notarisation arrive at beta.
- **All built-in adapters are Level 1 (Presence-only).** The attention engine therefore never emits `waiting_for_input` items for Aider / Codex / Claude Code sessions in the alpha — Level 2 classification (history-file mtime, log scraping) is planned for beta.
- **No per-adapter `/stop` override yet.** The platform-default mechanism is `SIGTERM` on Unix and `taskkill /F /PID` on Windows. SIGINT-aware TUI cleanup arrives when an adapter is promoted to Level 4.
- **Bot token stored plaintext in the local DB.** OS-keychain storage (macOS Keychain, Windows Credential Manager, Linux libsecret) arrives at beta.
- **No auto-update.** Alpha users update by reinstalling.
- **`usage_records` table is empty.** Cost surfaces a per-session `estimated` figure only when the custom adapter has a `cost_per_hour_cents` set; daily / weekly summaries and budget alerts arrive later.

## Reporting issues

Open an issue on GitHub. Useful information to include:

- AgentDeck build version (shown in the app menu).
- Linux distro and desktop environment (`lsb_release -a` and `echo "$XDG_CURRENT_DESKTOP / $XDG_SESSION_TYPE"`).
- A screenshot of the Diagnostics page when the issue is detection-related.
- The output of `AGENTDECK_LOG=debug agentdeck` for crashes or unexpected behaviour.

Do not attach `~/.local/share/agentdeck/agentdeck.db` if you have paired Telegram — the bot token is in plaintext in that file.

## Documentation

- [Install guide](docs/install.md) — `.deb` / `.AppImage` install, first-run notes, FUSE 2 caveat
- [Adapter capabilities](docs/adapter-capabilities.md) — per-adapter detection / status / usage / control matrix
- [Telegram setup](docs/telegram-setup.md) — pairing flow and full command reference
- [Privacy](docs/privacy.md) — what is local, what leaves your machine, what is stored
- [Security](docs/security.md) — threat model, secret handling, `/stop` invariants
- [Contributing](docs/contributing.md) — dev environment, workspace layout, conventions
- [Changelog](CHANGELOG.md) — per-release notes

## License

[MIT](LICENSE) © AgentDeck contributors.

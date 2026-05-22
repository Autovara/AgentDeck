# AgentDeck

> Local-first attention board and safe Telegram command center for AI coding agents.

AgentDeck is a small local app that shows the live status, current task, attention needs, and estimated cost of AI coding assistants running on your machine. It runs without an account, stores everything locally, and lets you safely control supported agents from Telegram — with confirmation required for anything destructive.

## Status

**Alpha — `0.1.0-alpha.1`.** Lead validation platform is Linux; macOS and Windows builds arrive at beta. The live gap list lives in [`docs/alpha-limitations.md`](docs/alpha-limitations.md).

## What it does

- Detects running terminal AI coding agents — **Claude Code**, **Codex CLI**, **Aider** — plus user-defined custom matchers.
- Classifies each session (`running`, `idle`, `waiting_for_input`, `rate_limited`, `stalled`, `errored`, `completed`, `unknown`) with confidence and source. Built-in adapters ship at Level 1 (Presence) in the alpha; Level 2+ classification lands later.
- Surfaces an attention board that prioritises sessions needing a human.
- Tracks per-session estimated cost from adapter-declared rates. Labels each cost as `exact`, `estimated`, or `unknown`.
- Lets you tag sessions by project or client and export the history to CSV / JSON.
- Exposes a safe Telegram surface: read-only commands (`/help`, `/status`, `/agents`, `/attention`, `/session`), `/mute` with audit, and `/stop` that requires a separate `STOP <id>` confirmation message.

## Platforms

- **Linux (alpha lead)** — `.deb` for Debian / Ubuntu, `.AppImage` portable binary. Tray surface where the desktop supports it (KDE Plasma, GNOME with the AppIndicator extension); falls back to dashboard + desktop notifications on stock GNOME and some Wayland sessions.
- macOS — menu-bar app. **Beta**.
- Windows — system tray app. **Beta**.

Live parity status: [`docs/alpha-limitations.md`](docs/alpha-limitations.md).

## Quick start

End-users want [`docs/install.md`](docs/install.md). Once you have the bundle:

```sh
# Debian / Ubuntu
sudo apt install ./agentdeck_0.1.0-alpha.1_amd64.deb

# Anywhere else on Linux
chmod +x ./agentdeck_0.1.0-alpha.1_amd64.AppImage
./agentdeck_0.1.0-alpha.1_amd64.AppImage
```

The app creates its database under `~/.local/share/agentdeck/agentdeck.db` on first run. Delete that file to start fresh.

## Building from source

Developers — start with [`docs/contributing.md`](docs/contributing.md). The short version:

```sh
# One-time prerequisites on Ubuntu 24.04 / similar:
sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev build-essential \
    pkg-config curl wget file
curl https://sh.rustup.rs -sSf | sh -s -- -y
npm i -g pnpm@11

# From the repo root:
pnpm install --frozen-lockfile
pnpm tauri:dev     # hot-reloading dev loop
pnpm tauri:build   # produces .deb + .AppImage under src-tauri/target/release/bundle/
```

CI runs the same gates: `cargo build / clippy -D warnings / test --workspace --locked` plus `pnpm typecheck`.

## Privacy

- Core monitoring is **local only**. AgentDeck does not phone home.
- When Telegram is enabled, AgentDeck connects to `api.telegram.org` and **only** that host. The bot token is stored locally; pairing is gated by a one-time code; only allowlisted Telegram user IDs receive any reply beyond the pair prompt.
- The sessions export writes to a file path you pick — no upload.

Detail: [`docs/privacy.md`](docs/privacy.md). Security model: [`docs/security.md`](docs/security.md).

## Documentation

- [Install guide](docs/install.md) — `.deb` / `.AppImage` installation, first-run notes
- [Alpha limitations](docs/alpha-limitations.md) — what works, what doesn't, and what's deferred to beta
- [Adapter capabilities](docs/adapter-capabilities.md) — per-adapter detection / status / usage / control matrix
- [Telegram setup](docs/telegram-setup.md) — pairing flow and command reference
- [Privacy](docs/privacy.md) — what is local, what leaves your machine, what is stored
- [Security](docs/security.md) — threat model, secret handling, remote-command invariants
- [Contributing](docs/contributing.md) — dev environment, workspace layout, conventions
- [Changelog](CHANGELOG.md) — per-release notes

## License

[MIT](LICENSE) © AgentDeck contributors.

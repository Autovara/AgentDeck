# AgentDeck

> Local-first, cross-platform attention board and Telegram command center for AI coding agents.

AgentDeck is a small local app that shows the live status, current task, attention needs, and estimated cost of AI coding assistants running on your machine. It works on macOS, Windows, and Linux, runs without an account, and lets you safely control supported agents from Telegram.

## Status

Pre-alpha. The live gap list lives in [`docs/alpha-limitations.md`](docs/alpha-limitations.md).

## What it does

- Detects running terminal AI coding agents (Claude Code, Codex CLI, Aider) on macOS, Windows, and Linux.
- Classifies each session as `running`, `idle`, `waiting_for_input`, `rate_limited`, `stalled`, `errored`, `completed`, or `unknown`, with confidence and source.
- Surfaces an attention board prioritizing sessions that need a human.
- Tracks estimated cost per session, project, and client, with `exact` / `estimated` / `unknown` labels.
- Exposes a safe Telegram mobile command surface: read-only commands plus a guarded `/stop` with confirmation and audit logging.

## Platforms

- macOS — menu-bar app
- Windows — system tray app
- Linux — app-indicator app where supported; falls back to dashboard window plus desktop notifications on stock GNOME and some Wayland sessions

The alpha is being validated on **Linux** first (Ubuntu 24.04 LTS, Fedora 41+, KDE Plasma 6), with macOS and Windows parity required by beta. Live parity status lives in [`docs/alpha-limitations.md`](docs/alpha-limitations.md).

## Quick start

The Tauri project skeleton is not in place yet. Build and run instructions will land in a later milestone.

## Documentation

- [Alpha limitations](docs/alpha-limitations.md)
- [Adapter capabilities](docs/adapter-capabilities.md)
- [Telegram setup](docs/telegram-setup.md)
- [Privacy](docs/privacy.md)
- [Security](docs/security.md)
- [Contributing](docs/contributing.md)

## License

[MIT](LICENSE) © AgentDeck contributors

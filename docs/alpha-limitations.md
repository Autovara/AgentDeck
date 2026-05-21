# Alpha Limitations

> Status: draft. Updated as the alpha is built.

This document tracks what does and does not work in the current alpha build. It is the source of truth for the gap between the current build and the V1 product.

## Lead platform and parity

The alpha is validated against a single lead platform with the other platforms continuing to build in CI. Full macOS, Windows, and Linux parity is required by beta.

- **Lead platform:** Linux (Ubuntu 24.04 LTS, Fedora 41+, KDE Plasma 6). Daily-driver validation target during the alpha.
- **macOS parity:** required by beta. Must compile and pass CI tests throughout the alpha.
- **Windows parity:** required by beta. Must compile and pass CI tests throughout the alpha.
- **Linux distro coverage outside the primary targets:** best-effort. Users on stock GNOME may need the AppIndicator extension to see a tray icon; the dashboard-plus-notifications fallback works without it.

## Adapters

See [`adapter-capabilities.md`](adapter-capabilities.md) for the live capability matrix.

| Agent | Detection | Status | Usage | Control | Notes |
|---|---|---|---|---|---|
| Claude Code | TBD | TBD | TBD | TBD | TBD |
| Codex CLI | TBD | TBD | TBD | TBD | TBD |
| Aider | TBD | TBD | TBD | TBD | TBD |
| Custom process | TBD | TBD | TBD | TBD | TBD |

## Telegram

- Pairing: _not yet implemented._
- Read-only commands: _not yet implemented._
- `/mute`: _not yet implemented._
- `/stop`: _not yet implemented._

## Tray and dashboard

- Tray availability detection: _not yet implemented._
- Dashboard fallback for tray-less desktops: _not yet implemented._

## Known issues

_None recorded yet._

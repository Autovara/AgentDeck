# Adapter Capabilities

> Status: draft. Filled in per adapter as adapters land.

Every adapter declares its capability level. This page tracks what each adapter actually supports in the current build, with its data sources and confidence.

## Capability levels

- **Level 1 — Presence.** Detects the process, identifies cwd or repo when available, shows running or completed state.
- **Level 2 — Status.** Detects waiting, idle, stalled, errored, or rate-limited state, with source and confidence.
- **Level 3 — Usage.** Extracts or estimates token and cost data, labeled `exact`, `estimated`, or `unknown`.
- **Level 4 — Control.** Supports safe permission-gated actions such as `stop`, `mute`, `continue`, or `approve`.

The alpha may ship adapters at Level 1 or Level 2, as long as the level is labeled honestly.

## Adapter matrix

| Adapter | Level | Detection sources | Status sources | Usage sources | Control actions | Confidence |
|---|---|---|---|---|---|---|
| Claude Code | TBD | TBD | TBD | TBD | TBD | TBD |
| Codex CLI | TBD | TBD | TBD | TBD | TBD | TBD |
| Aider | TBD | TBD | TBD | TBD | TBD | TBD |
| Custom process | 1 (target) | `sysinfo` process list, user-declared command pattern | n/a | n/a | none | high for presence, none for status |

## Known limitations

_To be filled per adapter._

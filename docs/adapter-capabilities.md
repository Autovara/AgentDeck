# Adapter Capabilities

> Status: alpha. Last updated for `0.1.0-alpha.1`.

Every adapter declares its capability level so the dashboard, the attention engine, and the Telegram bot can honestly tell the user what AgentDeck can and cannot say about each session.

## Capability levels

- **Level 1 — Presence.** Detects the process, identifies cwd or repo when available, shows running or completed state.
- **Level 2 — Status.** Detects waiting, idle, stalled, errored, or rate-limited state, with source and confidence.
- **Level 3 — Usage.** Extracts or estimates token and cost data, labeled `exact`, `estimated`, or `unknown`.
- **Level 4 — Control.** Supports safe permission-gated actions such as `stop`, `mute`, `continue`, or `approve`.

The alpha may ship adapters at any level as long as the level is labeled honestly. The attention engine **gates** `waiting_for_input` attention items to adapters at Level 2+ so Level 1 adapters never produce a misleading "waiting" signal from a TUI that is actually just thinking. See [`alpha-build-plan.md`](../planning/alpha-build-plan.md) §10 for the rationale.

## Adapter matrix (`0.1.0-alpha.1`)

| Adapter | Level | Detection signals | Status sources | Usage sources | Control actions | Confidence (alpha) |
|---|---|---|---|---|---|---|
| **Claude Code** (`agentdeck-adapter-claude-code`) | 1 | `name == claude\|claude-code`; `cmdline[0]` basename matches; `node` interpreter with positional `claude`/`claude-code` arg | _none — Level 2 pending_ | _none — Level 3 pending_ | _none — Level 4 pending; platform-default `SIGTERM`/`taskkill` via `/stop`_ | High for presence; Unknown elsewhere |
| **Codex CLI** (`agentdeck-adapter-codex`) | 1 | `name == codex`; `cmdline[0]` basename matches; `node` interpreter with positional `codex` arg (`node /path/codex`, `node --enable-source-maps /opt/codex/bin/codex`) | _none — Level 2 pending_ | _none — Level 3 pending_ | _none — Level 4 pending_ | High for presence; Unknown elsewhere |
| **Aider** (`agentdeck-adapter-aider`) | 1 | `name == aider`; `cmdline[0]` basename matches; `python*` interpreter with `-m aider[.<sub>]` or a positional path whose basename is `aider` | _none — Level 2 pending; Aider's `.aider.chat.history.md` mtime is the planned first signal_ | _none — Level 3 pending_ | _none — Level 4 pending_ | High for presence; Unknown elsewhere |
| **Custom process** (`agentdeck-adapter-custom`) | 1 | Three user-defined regex modes: against process name, joined command line, or `cwd` (processes with no `cwd` never match `cwd` rules) | _n/a — fixed at Level 1 per `planning/adapter-feasibility.md` §4.4_ | _user-supplied `cost_per_hour_cents`, applied to `(now - start_time)` for an `estimated` cost label_ | _none — `stop` uses the platform-default mechanism_ | Depends on the user's regex precision |

## Common limitations

- All adapters in the alpha are Level 1; the attention engine therefore never emits a `waiting_for_input` item — the rule fires only at level ≥ Status. See [`security.md`](security.md) and [`alpha-build-plan.md`](../planning/alpha-build-plan.md) §10.
- No adapter implements per-adapter `stop()` yet. `/stop` is dispatched by the Tauri shell's platform default: `SIGTERM` on Unix, `taskkill /F /PID` on Windows. Per-adapter SIGINT-aware cleanup arrives when an adapter is promoted to Level 4 (Control).
- Container-aware detection (Docker, podman, distrobox) is out of scope for the alpha. Processes inside containers appear in the host process list but their `cwd` does not match the host's view; the Diagnostics page surfaces a hint when this is detected.

## Known per-adapter limitations

### Claude Code

- The matcher catches `claude` *or* `claude-code`. The older `claude-code` binary name is kept as a fallback per `planning/adapter-feasibility.md` §4.1.
- `claude` is more generic than `aider` / `codex`; the matcher requires an *exact basename match* (not `starts_with`) so `claudette`, `claude-bot`, `pyclaude`, etc. do not trigger. If you hit a false positive, disable the adapter from the Diagnostics page and add a tighter custom matcher.

### Codex CLI

- The matcher requires an exact basename of `codex` (with or without `.exe`); `codex-cli`, `codexbot`, and `nodemon` are explicitly excluded.
- `node --enable-source-maps /opt/codex/bin/codex` is handled — flag arguments (anything starting with `-`) are skipped when scanning positional args.

### Aider

- The matcher recognises both the `pip install aider-chat` entrypoint (`aider` on `$PATH`) and `python -m aider[.cli]` style invocations, including venv-internal launches like `python /home/me/.venvs/aider/bin/aider`.
- `pythonista` (a mobile python clone) is explicitly excluded so it cannot impersonate `python` and trigger the `-m aider` branch.

### Custom process

- Each definition is locked to **Level 1**; the dashboard's "add custom adapter" form does not expose level selection.
- Two custom-adapter definitions matching the same process produce *one* session row (`adapter_name = "custom"`, `agent_name` = last writer wins). The Custom Adapters card shows the per-rule match list separately so the user can see which rule(s) hit each PID.
- `cost_per_hour_cents` is the only Level 3 hook in the alpha. When set, the state machine writes `estimated_cost = rate × (now - start) / 3600 / 100` on every tick and labels it `estimated`.

## What changes at Level 2

When an adapter is promoted to Level 2, the attention engine starts emitting `waiting_for_input` items for that adapter's sessions whenever the adapter reports `WaitingForInput`. To avoid surprises, the promotion ladder is:

1. The adapter starts reading at least one external signal (history file, log line, IPC handshake) and demonstrates it under fixtures in `agentdeck-harness`.
2. `Adapter::capability_level` returns `CapabilityLevel::Status`.
3. The adapter populates `AdapterMatch.status`, `status_confidence`, and `status_source` with non-default values.
4. The attention engine starts firing for sessions from that adapter without changes to the engine itself — the existing Level >= Status gate flips automatically.

The same gate applies to Level 3 (cost) and Level 4 (control); promotion is per-adapter and surfaces in the Diagnostics page.

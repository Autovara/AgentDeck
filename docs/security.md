# Security

> Status: draft.

This document describes the security model for AgentDeck, with particular focus on the Telegram remote-control surface.

## Threat model

To be filled. The core threats the alpha must defend against:

- an unauthorized Telegram user issuing remote commands
- an attacker on the network reading agent prompts or terminal output through AgentDeck
- accidental destructive remote actions (`/stop` on the wrong session)
- secrets leaking into notifications, audit logs, or chat messages

## Telegram remote control

Defaults the alpha must enforce:

- Telegram is disabled by default.
- Only allowlisted Telegram user IDs can issue commands.
- `/stop`, `/continue`, and `/approve` require per-command confirmation.
- Every remote command is written to the local audit log, including denied and unsupported commands.
- The user can revoke Telegram access locally without restarting the app.
- No arbitrary shell commands are accepted.
- Commands are rate-limited.
- The `/stop` action dispatches through the adapter and uses the platform-correct termination mechanism. Adapters that cannot map `Stop` to a known-safe action must report `stop_unsupported` instead of attempting termination.

### `/stop` confirmation invariants (alpha)

`/stop` is the only destructive remote command AgentDeck implements today. Every confirmed stop sends exactly one signal: `SIGTERM` on Unix, `taskkill /F /PID` on Windows. Per-adapter SIGINT-first cleanup lands when an adapter is promoted to capability Level 4.

The invariants enforced by `agentdeck-telegram` and `src-tauri/src/stop.rs`:

- The two-message protocol — `/stop <id>` followed by `STOP <id>` — is mandatory. There is no API path that skips the confirmation step.
- Pending confirmations are kept per Telegram user id, **in memory only**, with a fixed 60-second expiry. A crash wipes them.
- `STOP <code>` with no pending slot, mismatch, or expiry never sends a signal.
- `/stop` and `STOP` both write to `audit_log`. `stop.requested` is `audit_only`; `stop.executed` records `success`, `failed`, or `audit_only` depending on the dispatcher outcome. The dispatcher's `mechanism` (`SIGTERM` / `taskkill`) and any error message are stored verbatim in the row's metadata.
- The `remote_commands` table mirrors the lifecycle (`pending` → `executed` | `failed`) so the dashboard can render a stop history surface later.
- Sessions whose status is already `completed` short-circuit before any signal is sent.
- Sessions with `pid IS NULL` short-circuit as `Unsupported`.

## Secret handling

- Notifications and chat messages must not include raw secrets.
- Likely secrets are redacted before being persisted or sent.
- Users can opt in per session to capture redacted log snippets for diagnostics; this is off by default.

## Telegram bot token storage

The Telegram bot token grants full control of your bot. AgentDeck handles it as follows:

- **Alpha:** the token is stored as plain text in the local SQLite database (`settings.telegram.bot_token`). The database lives in your per-OS app data directory, the same trust boundary as `~/.ssh` and other user secrets.
- **Beta:** the token will move to the OS keychain (macOS Keychain / Windows Credential Manager / Linux libsecret) so it is no longer readable by other processes running as your user.
- The token is never logged, never written to the audit log, and never returned to the dashboard once saved. The dashboard only sees `hasToken: true|false`.
- Clearing or replacing the token from the dashboard stops the bot and writes an audit entry of the change (but not the token itself).

If you suspect a token has leaked, revoke it with `@BotFather`'s `/revoke` and save a new one in AgentDeck.

## Reporting a vulnerability

To be filled before the repo goes public.

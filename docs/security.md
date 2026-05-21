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

## Secret handling

- Notifications and chat messages must not include raw secrets.
- Likely secrets are redacted before being persisted or sent.
- Users can opt in per session to capture redacted log snippets for diagnostics; this is off by default.

## Reporting a vulnerability

To be filled before the repo goes public.

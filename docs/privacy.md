# Privacy

> Status: draft.

## Summary

- Core monitoring runs entirely on your machine.
- No account is required.
- No telemetry is sent by default.
- No source code, prompts, or terminal output leave your machine unless you explicitly enable an integration that sends it (for example, Telegram).

## What is stored locally

Data is stored under the platform-standard app data directory:

- macOS: `~/Library/Application Support/AgentDeck/`
- Windows: `%APPDATA%\AgentDeck\`
- Linux: `$XDG_DATA_HOME/agentdeck/` or `~/.local/share/agentdeck/`

Raw terminal output is not stored by default. Default retention is:

- session events: 90 days
- usage summaries: 13 months
- audit log: indefinite until you explicitly clear it
- raw terminal output snippets, when opt-in: 7 days

## What leaves your machine when Telegram is enabled

When Telegram is enabled, AgentDeck connects to `api.telegram.org` over HTTPS and long-polls for messages addressed to your bot. Specifically:

- **Outbound:** AgentDeck contacts only `api.telegram.org`. There is no other network destination. Connections are TLS via rustls; AgentDeck does not use system-installed OpenSSL on Linux.
- **Inbound:** AgentDeck reads messages sent to your bot. Pairing messages (`PAIR <code>`) and, in later alpha builds, command messages (`/status`, etc.) are processed locally; the message text itself is not stored or echoed elsewhere.
- **Reply content:** alert and command replies are routed through Telegram's servers. The product redacts likely secrets by default and prefers minimal alert content.
- **Bot allowlist:** only Telegram user IDs you have paired through the AgentDeck app receive any reply other than the pairing prompt. The bot token alone is not enough — pairing is required.
- **Disable at any time:** the dashboard's Settings page stops the bot immediately; AgentDeck stops contacting Telegram until you re-enable it.

## Data export and deletion

The dashboard's Settings page provides a sessions export to either CSV or JSON. Both formats include every session row (active and completed) with its agent name, adapter, repo path, project tag, status, cost label, and timestamps. **Bot tokens, audit-log entries, attention messages, and raw terminal output are not included** in the sessions export.

Export writes go straight to a file path you pick via the OS save dialog. AgentDeck never uploads exported data anywhere — the file lives wherever you save it.

The audit log is preserved until you explicitly clear it. A clear-history action will land in a later alpha build.

## Reporting a privacy concern

To be filled in before the repo goes public.

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

When Telegram is enabled, alert and command content is routed through the Telegram service you are connected to. The product redacts likely secrets by default and prefers minimal alert content. Specifics will be filled in when Telegram pairing is implemented.

## Data export and deletion

The dashboard provides export to CSV or JSON and a clear-history action with confirmation. The audit log is preserved until you explicitly clear it.

## Reporting a privacy concern

To be filled in before the repo goes public.

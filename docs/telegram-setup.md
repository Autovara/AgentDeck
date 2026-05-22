# Telegram Setup

> Status: alpha. Pairing is implemented; read-only commands and `/mute` / `/stop` arrive in later alpha builds.

Telegram is **off by default**. Until you complete the steps below, AgentDeck never contacts `api.telegram.org`.

## 1. Create a bot with @BotFather

1. Open Telegram and start a chat with [@BotFather](https://t.me/BotFather).
2. Send `/newbot`.
3. Pick a display name and a username for your bot (the username must end with `bot`).
4. BotFather replies with an HTTP API token of the form `123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11`. Treat this like a password — anyone who has it can act as your bot.

## 2. Save the token in AgentDeck

1. Open the AgentDeck dashboard and go to **Settings**.
2. Click **Enable Telegram**.
3. Paste the token into the **Token** field and click **Save token**. AgentDeck starts the bot in the background and begins listening for messages.

The token is stored locally — see [`security.md`](security.md) for details on how.

## 3. Pair your Telegram user

1. In the dashboard, click **Generate pairing code**. AgentDeck shows a 6-character code (for example `K7M2QH`) that expires in 10 minutes.
2. Open Telegram on your phone or desktop and find your bot.
3. Send the bot a message of the form:

   ```
   PAIR K7M2QH
   ```

4. The bot replies `✓ Paired as @yourhandle` (or `id <number>` if you have no @-handle). Your Telegram user is now on the allowlist.

If the code expired or you mistyped it, click **Generate pairing code** again — each click invalidates the previous code.

## 4. Pair more users (optional)

Generate a new code for every additional user. Each code is single-use; AgentDeck never lists a user as paired until they have completed the handshake from their own Telegram account.

## 5. Revoke access

- **Remove one user:** open Settings, find them in the Paired Users table, and click **Revoke**. They will see no further replies beyond the unpaired prompt.
- **Disable Telegram entirely:** click **Disable Telegram**. The bot stops immediately. Re-enabling it later does not require re-pairing.
- **Delete the token:** click **Clear token**. The bot stops; existing pairings are kept in case you save a new token under the same bot later.

All four actions write to the local audit log (see [`security.md`](security.md)).

## Available commands

The alpha pairing build only handles `PAIR <code>`. Once paired, every other message gets a stub reply (`Commands are not implemented yet`). The full command set lands in later alpha builds:

- `/help`
- `/status`
- `/agents`
- `/attention`
- `/session <id>`
- `/mute <id>`
- `/stop <id>` (with confirmation, per [`security.md`](security.md))

## Troubleshooting

- **"Pairing succeeded but AgentDeck failed to record it"** — the local database write failed. Open the Diagnostics page in AgentDeck and check the Storage card for the error. Generate a new code and try again.
- **The bot never replies** — confirm the token is saved (Settings shows "Token saved: Yes") and the bot is running ("Bot running: Yes"). If running is `No`, AgentDeck logged a startup error; check the app logs (the `AGENTDECK_LOG` env var controls verbosity).
- **"AgentDeck is not paired with you"** — your Telegram user ID is not on the allowlist. Generate a fresh code and DM `PAIR <code>` from the account you want to pair.

---
name: Bug report
about: Something AgentDeck got wrong — a crash, a missed or false detection, a broken action
title: ""
labels: bug
assignees: ""
---

**What happened**

A clear description of the bug.

**What you expected instead**

**Steps to reproduce**

1.
2.
3.

**Environment**

- AgentDeck version (from the app menu):
- Install method: `.deb` / `.AppImage`
- Distro + version (`lsb_release -a`):
- Desktop + session (`echo "$XDG_CURRENT_DESKTOP / $XDG_SESSION_TYPE"`):
- Which agent(s) involved: Aider / Codex CLI / Claude Code / custom / n/a

**Diagnostics**

If this is a detection problem (an agent not showing up, or the wrong thing showing
up), attach a screenshot of the **Diagnostics** page.

**Logs**

For a crash or unexpected behaviour, paste the relevant output of:

```sh
AGENTDECK_LOG=debug agentdeck-app
```

> Do **not** attach `~/.local/share/agentdeck/agentdeck.db` if you have paired
> Telegram — your bot token is stored there in plaintext during the alpha.

# Installing the AgentDeck alpha

AgentDeck's alpha ships as **Linux-only** `.deb` and `.AppImage` artifacts. macOS and Windows builds arrive at beta — see [`alpha-limitations.md`](alpha-limitations.md).

The alpha binaries are **unsigned**. Linux does not have a Gatekeeper-equivalent prompt, but your distro will refuse the install if the dependency list is not satisfied; the lists below should keep that simple.

## Producing the artifacts

You only need to do this if you are cutting your own build. End-users get the artifacts from the alpha release page (TBD).

```sh
# Prerequisites (one-time, Ubuntu 24.04 / Fedora 41+ / similar):
sudo apt install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    build-essential \
    pkg-config \
    curl \
    wget \
    file
# Rust + Node:
curl https://sh.rustup.rs -sSf | sh -s -- -y
npm i -g pnpm@11

# From the repo root:
pnpm install --frozen-lockfile
pnpm tauri build
```

The two output artifacts land in `src-tauri/target/release/bundle/`:

- `deb/agentdeck_0.1.0-alpha.1_amd64.deb` — Debian / Ubuntu package
- `appimage/agentdeck_0.1.0-alpha.1_amd64.AppImage` — portable single-file binary

The release-mode build is configured with `lto = "thin"`, `codegen-units = 1`, and `strip = true`, so the bundles are small (~12 MB AppImage on a typical Linux box; varies with WebKit version).

## Installing from `.deb`

```sh
sudo apt install ./agentdeck_0.1.0-alpha.1_amd64.deb
```

The `.deb` declares the following runtime dependencies; `apt` resolves them automatically:

- `libwebkit2gtk-4.1-0` — the WebKit engine Tauri renders the dashboard with.
- `libgtk-3-0` — the desktop toolkit Tauri uses on Linux.
- `libayatana-appindicator3-1` — the tray surface AgentDeck prefers on KDE, GNOME (with the extension), Cinnamon, MATE, etc.

After installation:

- Launch from your application menu (it appears under "Development" — set by `category: DeveloperTool`).
- Or run `agentdeck` from a terminal.

Uninstall: `sudo apt remove agentdeck`.

## Installing from `.AppImage`

The AppImage is a self-contained binary — handy when you don't want to add a package, or when your distro is not Debian-family.

```sh
chmod +x agentdeck_0.1.0-alpha.1_amd64.AppImage
./agentdeck_0.1.0-alpha.1_amd64.AppImage
```

You may see a prompt the first time you run it asking whether to integrate it with your desktop environment — that's the AppImage launcher, not AgentDeck. Either choice is fine.

### AppImage caveats

- **FUSE 2 is required.** Most desktop distros ship it. If you see `dlopen(): error loading libfuse.so.2`, install `libfuse2` (`sudo apt install libfuse2t64` on recent Ubuntu).
- The AppImage carries its own copy of WebKit, so it's larger than the `.deb` and starts a little slower on first launch.
- It is **not** sandboxed — the same filesystem and network permissions as any other binary you'd run.

## First-run notes

On first launch AgentDeck creates its database under the per-OS app data directory:

- Linux: `$XDG_DATA_HOME/agentdeck/agentdeck.db` (defaults to `~/.local/share/agentdeck/agentdeck.db`)

The data directory is recreated automatically if you delete it; you'll lose any saved Telegram tokens, paired users, and session history.

If the tray surface isn't available on your desktop (stock GNOME without the AppIndicator extension, some Wayland sessions), AgentDeck falls back to opening the dashboard window directly — see [`adapter-capabilities.md`](adapter-capabilities.md) and the Diagnostics page in the app for what AgentDeck detected.

## Reporting build issues

Open an issue on GitHub with:

- Your Linux distro + version (`lsb_release -a`)
- Output of `dpkg -l | grep webkit` (or your distro's equivalent)
- The contents of `~/.local/share/agentdeck/agentdeck.db` is **safe to share** (no tokens are stored in plaintext on the alpha if you have not enabled Telegram), but please double-check it does not contain a bot token you care about before posting.

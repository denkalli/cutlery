# Cutlery

[![Platform: Linux](https://img.shields.io/badge/platform-Linux-blue.svg)](https://kernel.org)
[![Wine: 9.x / 10.x / 11.x](https://img.shields.io/badge/wine-11.x%20compatible-orange.svg)](https://winehq.org)
[![MCP: Supported](https://img.shields.io/badge/MCP-Universal%20Bridge-brightgreen.svg)](https://modelcontextprotocol.io)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

A lightweight, high-performance **Roblox Studio launcher for Linux** with a native rootless fix for the multi-window panel docking bug, an Antigravity-inspired pre-launch Control Center, and a **Universal Model Context Protocol (MCP) Bridge** for AI agents (**Claude, Cursor, Codex, Antigravity**).

---

## Features

- **Control Center GUI**: Modern pre-launch dashboard for graphics, kernel sync, custom FastFlags, and AI bridge management.
- **Themes**: Built-in dark, light, Midnight and OLED themes, plus your own TOML themes with live preview, optionally applied to Roblox Studio too (experimental).
- **Rootless Docking Fix**: Native fix for Qt/Win32 panel docking without requiring a Virtual Desktop.
- **Universal MCP Bridge**: Direct stdio JSON-RPC bridge connecting AI coding assistants to Roblox Studio.
- **Discord Rich Presence**: Shows the experience you are editing or playtesting on your Discord profile, with an optional privacy mode.
- **Vulkan & Low-Latency Sync**: High-performance DXVK Direct3D 11 rendering with NTSync (`/dev/ntsync`) and `winefsync` support.
- **Automated Provisioning**: Built-in Studio installer, prefix management, and desktop integration.

---

## Quick Start

### Prerequisites
- **Linux x86_64**
- **Wine 9.0+** (or Wine-Staging)
- **Vulkan Drivers** (Mesa RADV / ANV or NVIDIA)
- **Python 3.8+** (for MCP Bridge)

### Installation & Run

Every [GitHub Release](https://github.com/denkalli/cutlery/releases) ships three packages, each with a `.sha256` file:

| Package | Install |
| --- | --- |
| `Cutlery-vX.Y.Z-x86_64.AppImage` | `chmod +x Cutlery-*.AppImage && ./Cutlery-*.AppImage` |
| `cutlery_X.Y.Z_amd64.snap` | `sudo snap install --classic --dangerous cutlery_*.snap` |
| `cutlery-vX.Y.Z-linux-x86_64.tar.gz` | Extract anywhere and run `./cutlery` |

The AppImage and the Snap are read-only, so they keep the Wine prefix and DXVK in `~/.local/share/cutlery/sandbox/` and update by installing the newer package instead of using the Update button. The Snap uses classic confinement because Wine, your prefix and the Discord socket live on the host. All packages use the host's Wine, Python 3 and curl. To build from source instead:

```bash
# 1. Clone & build
git clone https://github.com/denkalli/cutlery.git
cd cutlery
cargo build --release

# 2. Launch Control Center GUI
./target/release/cutlery

# 3. Or launch Roblox Studio directly
./target/release/cutlery run
```

*(Optional)* Copy `./target/release/cutlery` to `~/.local/bin/` (or use the AppImage/Snap) and run `cutlery register-desktop` to integrate with your desktop app launcher and `roblox-studio://` browser links.

---

## AI Agent Integration (MCP)

Cutlery provides a unified MCP server endpoint via `cutlery mcp`.

Once Roblox Studio closes, the bridge stops its `StudioMCP.exe` and exits, so idle AI clients don't keep Wine running in the background. Your client reconnects the next time it starts the server. To keep the bridge running instead, turn off **Universal MCP → Close With Studio**, set `exit_with_studio = false` under `[mcp]` in `~/.config/cutlery/config.toml`, or run `cutlery mcp --keep-alive`.

### Claude Desktop (`~/.config/Claude/claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "roblox-studio": {
      "command": "cutlery",
      "args": ["mcp"]
    }
  }
}
```

### Cursor (`~/.cursor/mcp.json`)
```json
{
  "mcpServers": {
    "roblox-studio": {
      "command": "cutlery",
      "args": ["mcp"]
    }
  }
}
```

### Antigravity
Link the plugin with 1-click in the **Universal MCP** tab of the Cutlery Control Center, or copy `plugins/antigravity-roblox` into `~/.gemini/config/plugins/roblox-studio`.

---

## Themes

Pick a theme in **Graphics → Appearance**. To make your own, drop a `.toml` file into `~/.config/cutlery/themes/` (the **Open Themes Folder** button creates it) and press **Reload Themes**. Any color you leave out is taken from the base palette, so a theme can be as short as:

```toml
name = "Dracula"
author = "you"
base = "dark"            # or "light"
# background = "bg.png"  # optional Home artwork, relative to this file

[colors]
window-bg = "#282a36"
surface = "#343746"
surface-sunken = "#21222c"
accent = "#ff79c6"       # optional; without it the accent chips stay selectable
```

The available colors are the keys in [`data/themes/cutlery-dark.toml`](data/themes/cutlery-dark.toml). A user theme named like a built-in one (for example `oled.toml`) replaces it.

### Roblox Studio (experimental)

Turn on **Apply theme to Roblox Studio** in **Graphics → Appearance** and launch Studio from Cutlery. The selected theme then recolors Studio's panels, menus, tabs, Properties, Output and the Script Editor. The ribbon, Explorer and Toolbox are built-in Luau plugins and keep Studio's own colors.

How it works: Studio reads its theme from JSON files compiled into `RobloxStudioBeta.exe`. Cutlery writes a copy, `RobloxStudioCutlery.exe`, that differs in two bytes and loads those files from the version folder instead, then writes them remapped to your theme. The original exe is never modified, Studio's Dark/Light setting is switched to match the theme, and everything is redone automatically after a Studio update. If anything fails, Studio starts normally with its own theme. The **Doctor** page shows whether the installed Studio build can be themed. Details: [`docs/studio-themes.md`](docs/studio-themes.md).

Themes can fine-tune Studio with an optional `[studio]` table:

```toml
[studio.remap]            # original Studio color -> new color
"#353741" = "#44475a"

[studio.colors]           # StudioStyleGuideColor, optionally with a modifier
MainBackground = "#21222c"
"Item.Hover" = "#44475a"

[studio.syntax]           # Script Editor colors, applied to every preset
Keyword = "#ff79c6"
String = "#f1fa8c"
```

---

## Discord Rich Presence

While Studio runs through `cutlery run` (or the Control Center's Launch button), Cutlery follows Studio's log and shows on Discord what you are doing: **Editing Baseplate · Building**, **Scripting**, **Testing Baseplate · Playtesting**, or **In Roblox Studio · Start page**, with the elapsed time and a "View on Roblox" button. The experience name comes from Roblox's public API.

Configure it in **Universal MCP → Discord Rich Presence** or in `~/.config/cutlery/config.toml`:

```toml
[discord]
enabled = true
privacy_mode = false      # true: "Editing a place", no name and no Roblox link
show_elapsed = true
show_buttons = true
application_id = ""       # optional: your own Discord application (see docs/discord-rpc.md)
```

The native Discord client, its Flatpak and Snap builds and Vesktop/arRPC are supported. If Discord is not running, Studio starts normally and the presence appears once Discord is opened. Details in [docs/discord-rpc.md](docs/discord-rpc.md).

---

## CLI Reference

| Command | Description |
| :--- | :--- |
| `cutlery [ui]` | Opens the Control Center GUI (default). |
| `cutlery run [args...]` | Launches Roblox Studio with docking fix and DXVK optimizations. |
| `cutlery mcp [args...]` | Runs the Universal MCP Bridge over `stdio`. |
| `cutlery install` | Installs or updates Roblox Studio in the Wine prefix. |
| `cutlery update` | Updates Cutlery from GitHub Releases (with confirmation), then Roblox Studio if a newer build exists. |
| `cutlery register-desktop` | Registers desktop launcher and `roblox-studio://` URI handler. |
| `cutlery doctor` | Diagnostic health check (Wine, Vulkan, NTSync, Prefix). |

---

## Acknowledgments

We express gratitude to the open-source projects that pioneered Roblox gaming and development on Linux:
- **[Vinegar](https://github.com/vinegarhq/vinegar)**: For modern Wine/DXVK configurations and FastFlags strategies.
- **[Grapejuice](https://gitlab.com/brinkervii/grapejuice)**: For pioneering native Roblox Studio on Linux.
- **[DXVK](https://github.com/doitsujin/dxvk)**: Direct3D 11 to Vulkan translation and DirectComposition swapchains.
- **[WineHQ](https://www.winehq.org)**: Windows compatibility on POSIX systems.

---

## License

Cutlery is licensed under the **[GNU General Public License v3.0 (GPL-3.0)](LICENSE)**. As open-source software, full implementation details and source code can be freely inspected.

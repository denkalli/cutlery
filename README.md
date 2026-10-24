# Cutlery

[![Platform: Linux](https://img.shields.io/badge/platform-Linux-blue.svg)](https://kernel.org)
[![Wine: 9.x / 10.x / 11.x](https://img.shields.io/badge/wine-11.x%20compatible-orange.svg)](https://winehq.org)
[![MCP: Supported](https://img.shields.io/badge/MCP-Universal%20Bridge-brightgreen.svg)](https://modelcontextprotocol.io)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

A lightweight, high-performance **Roblox Studio launcher for Linux** with a native rootless fix for the multi-window panel docking bug, an Antigravity-inspired pre-launch Control Center, and a **Universal Model Context Protocol (MCP) Bridge** for AI agents (**Claude, Cursor, Codex, Antigravity**).

---

## Features

- **Control Center GUI**: Modern pre-launch dashboard for graphics, kernel sync, custom FastFlags, and AI bridge management.
- **Rootless Docking Fix**: Native fix for Qt/Win32 panel docking without requiring a Virtual Desktop.
- **Universal MCP Bridge**: Direct stdio JSON-RPC bridge connecting AI coding assistants to Roblox Studio.
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

*(Optional)* Copy `./target/release/cutlery` to `~/.local/bin/` and run `cutlery register-desktop` to integrate with your desktop app launcher and `roblox-studio://` browser links.

---

## AI Agent Integration (MCP)

Cutlery provides a unified MCP server endpoint via `cutlery mcp`.

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

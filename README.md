# Cutlery

[![Platform: Linux](https://img.shields.io/badge/platform-Linux-blue.svg)](https://kernel.org)
[![Wine: 9.x / 10.x / 11.x](https://img.shields.io/badge/wine-11.x%20compatible-orange.svg)](https://winehq.org)
[![MCP: Supported](https://img.shields.io/badge/MCP-Universal%20Bridge-brightgreen.svg)](https://modelcontextprotocol.io)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

A lightweight, high-performance **Roblox Studio launcher for Linux**, featuring a native rootless fix for the **multi-window panel docking bug** (Qt / Win32) without Virtual Desktop, plus a **Universal Model Context Protocol (MCP) Bridge** to connect AI agents (**Claude Desktop, Cursor, Codex, Antigravity**) directly to the Studio DataModel.

---

## Key Features

### 1. Native Docking Window Fix (Rootless Multi-Surface)
- **Zero Virtual Desktop Required**: Preserve native KDE Plasma, GNOME, or X11 multi-monitor ergonomics.
- **Continuous Motion Tracking**: Intercepts `_NET_WM_MOVERESIZE` during window movement, routing the move loop into Win32 `sys_command_size_move` to feed real-time `WM_MOVING` coordinates directly to Studio's `QtitanDocking` engine.
- **DirectComposition Swapchain Emulation**: Supplies dummy DirectComposition swapchains via DXVK (`dxgi.enableDummyCompositionSwapchain = true`), allowing docking guide markers and blue preview overlays to render smoothly.
- **Universal Dynamic Hook**: Uses `wine_hook.so` via `LD_PRELOAD` without requiring custom Wine builds or root privileges.

### 2. Universal MCP Bridge
- **Universal AI Agent Connectivity**: Connects **Claude Desktop**, **Cursor**, **Codex**, **Windsurf**, and **Antigravity** to Roblox Studio via standard Model Context Protocol.
- **Pre-Initialization Shield**: Handles speculative pre-init probes (e.g. Antigravity's `server/discover`) locally with standard JSON-RPC 2.0 error responses, ensuring `StudioMCP.exe` always receives `initialize` as its first packet.
- **Version Discovery**: Automatically detects the active `StudioMCP.exe` within the Wine prefix across Roblox updates.
- **Wine Stdio Isolation**: Suppresses Wine debug output (`WINEDEBUG=-all`) to keep JSON-RPC streams 100% clean.

### 3. High-Performance Rendering & Security
- **DXVK Direct3D 11 to Vulkan**: Asynchronous shader pipeline compilation with Graphics Pipeline Library (GPL).
- **Low-Latency Sync**: Full support for `/dev/ntsync` (Linux Kernel NTSync driver) and `winefsync` (futex2).
- **Hardened Package Verification**: Verified cryptographic checksums (MD5 for Roblox packages and SHA-256 for DXVK) and Zip Slip path traversal defenses.

---

## Architecture

```
+-----------------------------------------------------------------------------------------+
|                                       HOST LINUX                                        |
|                                                                                         |
|   +-----------------------+              +------------------------------------------+   |
|   |   AI Agent Client     |              |           Desktop Environment            |   |
|   | (Claude / Antigrav /  |              |        (KDE Plasma / GNOME / etc.)       |   |
|   |  Cursor / Codex)      |              +------------------------------------------+   |
|   +-----------+-----------+                                   ^                         |
|               | (JSON-RPC via stdio)                          | (X11 / Xwayland)        |
|               v                                               v                         |
|   +-----------------------+              +------------------------------------------+   |
|   |   mcp/bridge.py       |              |                 cutlery                  |   |
|   | (Pre-init shield &    |              |       (LD_PRELOAD wine_hook.so)          |   |
|   |  prefix discoverer)   |              +--------------------+---------------------+   |
|   +-----------+-----------+                                   |                         |
|               | (Spawns under Wine)                           | (Launches Studio)       |
|               v                                               v                         |
|  ============================== WINEPREFIX ENVIRONMENT ==============================   |
|               |                                               |                         |
|   +-----------v-----------+   Win32 Named Pipe (IPC)   +------v---------------------+   |
|   |     StudioMCP.exe     | <========================> |     RobloxStudioBeta.exe   |   |
|   |   (MCP Server Proxy)  |   (\\\\.\\pipe\\...)       |    (QtitanDocking / DXVK)  |   |
|   +-----------------------+                            +----------------------------+   |
+-----------------------------------------------------------------------------------------+
```

---

## Quick Start

### Prerequisites
- **Operating System**: Linux x86_64
- **Wine**: Wine 9.0+ / 10.x / 11.x (or Wine-Staging)
- **Vulkan Driver**: NVIDIA (550+), Mesa RADV (AMD), or Mesa ANV (Intel)
- **Python**: Python 3.8+ (for MCP Bridge)

### Installation & Build

1. Clone this repository:
   ```bash
   git clone https://github.com/denkalli/cutlery.git
   cd cutlery
   ```

2. Compile Cutlery:
   ```bash
   cargo build --release
   ```
   *(Optional)* Copy the binary to your `PATH` (e.g. `~/.local/bin/`):
   ```bash
   cp target/release/cutlery ~/.local/bin/
   ```

3. Launch Cutlery Control Center:
   ```bash
   cutlery
   # Or using local build: ./target/release/cutlery
   ```
   *Opens the modern Control Center GUI dashboard to configure Graphics, Kernel synchronization, FastFlags, and AI MCP integrations.*

4. Launch Roblox Studio directly:
   ```bash
   cutlery run
   # Or using local build: ./target/release/cutlery run
   ```

5. (Optional) Register desktop entry and browser protocol:
   ```bash
   cutlery register-desktop
   ```
   *Registers the application in your desktop launcher (opening the Control Center) and associates `roblox-studio://` browser URI links.*

---

## MCP Integration

### Connecting Claude Desktop
Add the following to `~/.config/Claude/claude_desktop_config.json`:

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

### Connecting Cursor
Add the following to `~/.cursor/mcp.json`:

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

### Connecting Antigravity
You can link the Antigravity plugin with 1-click in the **Universal MCP** dashboard tab, or configure it manually:

1. Copy the plugin:
   ```bash
   mkdir -p ~/.gemini/config/plugins/
   cp -r plugins/antigravity-roblox ~/.gemini/config/plugins/roblox-studio
   ```

2. In the Cutlery Control Center under **Universal MCP**, click **Link Antigravity Plugin** (or verify that `mcp_config.json` specifies `"command": "cutlery"`).

---

## CLI Reference

The `cutlery` command-line interface provides the following commands:

| Command | Description |
| :--- | :--- |
| `cutlery [ui]` | Opens the Settings & Control Center GUI dashboard (default when invoked without arguments). |
| `cutlery run [args...]` | Launches Roblox Studio with the docking fix, DXVK DirectComposition swapchains, and low-latency sync. |
| `cutlery mcp [args...]` | Starts the Universal MCP Bridge on `stdio` for AI agent connectivity. |
| `cutlery install` | Runs the automated Roblox Studio installer and provisions the Wine prefix. |
| `cutlery register-desktop` | Registers desktop launcher and associates `roblox-studio://` browser URI handler. |
| `cutlery doctor` | Diagnostic tool checking Wine, NTSync, Vulkan, Driver, and Prefix health. |
| `cutlery help` | Displays command-line help summary. |

### Environment Variables
- `WINEPREFIX`: Specify custom Wine prefix path (default: `sandbox/prefix`).
- `WINE`: Custom Wine binary to execute (e.g. `wine`, `wine-staging`, or custom runner).
- `WINE_STUDIO_DOCKING_FIX`: Set to `1` (default) to enable docking fix hooks.

---

## Acknowledgments

We express gratitude to the open-source projects that pioneered Roblox gaming and development on Linux:
- **[Vinegar](https://github.com/vinegarhq/vinegar)**: For modern Wine/DXVK configurations and FastFlags strategies.
- **[Grapejuice](https://gitlab.com/brinkervii/grapejuice)**: For proving running Roblox Studio natively on Linux was possible.
- **[DXVK](https://github.com/doitsujin/dxvk)**: For Direct3D 11 to Vulkan translation and DirectComposition swapchains.
- **[WineHQ](https://www.winehq.org)**: For Windows compatibility on POSIX systems.

---

## License
This project is licensed under the **GNU General Public License v3.0 (GPL-3.0)** - see the [LICENSE](LICENSE) file for details.

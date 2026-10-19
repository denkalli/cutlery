# Cutlery

[![Platform: Linux](https://img.shields.io/badge/platform-Linux-blue.svg)](https://kernel.org)
[![Wine: 9.x / 10.x / 11.x](https://img.shields.io/badge/wine-11.x%20compatible-orange.svg)](https://winehq.org)
[![MCP: Supported](https://img.shields.io/badge/MCP-Universal%20Bridge-brightgreen.svg)](https://modelcontextprotocol.io)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

A lightweight, high-performance **Roblox Studio launcher for Linux**, featuring a native rootless fix for the chronic **multi-window panel docking bug** (Qt / Win32) without Virtual Desktop, plus a **Universal Model Context Protocol (MCP) Bridge** to connect AI agents (**Claude Desktop, Cursor, Codex, Antigravity**) directly to the Studio DataModel.

---

##  Key Features

### 1. Native Docking Window Fix (Rootless Multi-Surface)
- **Zero Virtual Desktop Required**: Keep your native KDE Plasma, GNOME, or X11 multi-monitor desktop ergonomics.
- **Continuous Motion Tracking**: Bypasses X11 `_NET_WM_MOVERESIZE` window manager pointer grabs during window movement (`SC_MOVE`), routing the move loop into Win32 `sys_command_size_move` to feed real-time `WM_MOVING` coordinates directly to Studio's `QtitanDocking` engine.
- **DirectComposition Swapchain Emulation**: Supplies dummy DirectComposition swapchains via DXVK (`dxgi.enableDummyCompositionSwapchain = true`), allowing QtitanDocking blue drop-target preview overlays and docking guide markers to render without crashing.
- **Seamless Hooking**: Injects patched `winex11.so` routines dynamically via `wine_hook.so` (`LD_PRELOAD`), requiring no root privileges and no custom system Wine reinstallation.

### 2. Universal MCP Bridge
- **Universal AI Agent Compatibility**: Connects **Claude Desktop**, **Cursor**, **Codex**, **Windsurf**, or **Antigravity** to Roblox Studio via the Model Context Protocol.
- **Pre-Initialization Shield**: Intercepts speculative pre-initialization probes (like Antigravity's `server/discover`) and replies locally with standard JSON-RPC 2.0 `-32601 Method not found`, guaranteeing that Roblox's strict `StudioMCP.exe` always receives `initialize` as its very first packet.
- **Dynamic Version Discovery**: Automatically locates the newest `version-*/StudioMCP.exe` in the Wine prefix across Roblox weekly updates.
- **Wine Stdio Noise Isolation**: Forces `WINEDEBUG=-all` on the Wine process to ensure a 100% pristine JSON-RPC stream over `stdout`.

### 3. High-Performance Rendering
- **DXVK 2.x / 3.x Direct3D 11 to Vulkan**: Asynchronous shader pipeline compilation (`dxvk.enableAsync = true`) with Graphics Pipeline Library (`dxvk.gpl = true`).
- **Low-Latency Kernel Synchronization**: Full support for `/dev/ntsync` (Linux Kernel NTSync driver) and `winefsync` (futex2).
- **FastFlags Injection**: Automatically injects optimized Direct3D 11 configurations (`ClientAppSettings.json`) preventing telemetry stalls and OpenGL fallbacks.

---

##  Architecture

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
|   |   mcp/bridge.py       |              |             bin/cutlery                  |   |
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

##  Quick Start

### Prerequisites
- **Operating System**: Linux x86_64 (Arch Linux, CachyOS, Fedora, Ubuntu 22.04+, etc.)
- **Wine**: Wine 9.0+ / 10.x / 11.x (or Wine-Staging)
- **Vulkan Driver**: Proprietary NVIDIA driver (550+), Mesa RADV (AMD), or Mesa ANV (Intel)
- **Python**: Python 3.8+ (for the MCP Bridge)

### Installation & Build
1. Clone this repository:
   ```bash
   git clone https://github.com/denkalli/clutery.git
   cd clutery
   ```

2. Compile Cutlery (Rust binary):
   ```bash
   cargo build --release
   mkdir -p bin
   cp target/release/cutlery bin/cutlery
   ```

3. Install / Initialize Roblox Studio in the prefix:
   ```bash
   ./bin/cutlery install
   ```

4. Launch Roblox Studio:
   ```bash
   ./bin/cutlery run
   ```
   *Any undocked panel (Explorer, Properties, Output, etc.) can now be dragged over any docking zone or panel and dropped to snap back into position natively!*

5. (Optional) Register desktop entry and browser protocol:
   ```bash
   ./bin/cutlery register-desktop
   ```
   *Now opening places or logging in via your browser directly invokes Cutlery!*

---

##  MCP Integration

### Connecting Claude Desktop
Add the following to your `~/.config/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "roblox-studio": {
      "command": "/absolute/path/to/Cutlery/bin/cutlery",
      "args": ["mcp"]
    }
  }
}
```

### Connecting Cursor
Add the following to your `~/.cursor/mcp.json` (or `.cursor/mcp.json` in your workspace):

```json
{
  "mcpServers": {
    "roblox-studio": {
      "command": "/absolute/path/to/Cutlery/bin/cutlery",
      "args": ["mcp"]
    }
  }
}
```

### Connecting Antigravity
You can use the bundled Antigravity plugin which includes **AI Rules & Luau Guardrails**:

1. **Option A: Workspace Plugin (Per Project)**
   Copy the `plugins/antigravity-roblox` directory to your project's `.agents/plugins/`:
   ```bash
   mkdir -p /path/to/your/project/.agents/plugins/
   cp -r plugins/antigravity-roblox /path/to/your/project/.agents/plugins/roblox-studio
   ```

2. **Option B: Global Plugin**
   Copy to your global Antigravity plugins directory:
   ```bash
   mkdir -p ~/.gemini/config/plugins/
   cp -r plugins/antigravity-roblox ~/.gemini/config/plugins/roblox-studio
   ```

3. Verify that `mcp_config.json` inside the plugin points to your absolute path of `./bin/cutlery mcp`.

---

##  CLI Reference

The compiled binary `./bin/cutlery` provides the following commands:

| Command | Description |
| :--- | :--- |
| `./bin/cutlery run [args...]` | Launches Roblox Studio with the rootless docking patch, DXVK DirectComposition dummy swapchains, and low-latency sync. |
| `./bin/cutlery mcp [args...]` | Starts the Universal MCP Bridge on `stdio` for AI agent connectivity. |
| `./bin/cutlery install` | Runs the automated Roblox Studio installer and provisions the Wine prefix. |
| `./bin/cutlery register-desktop` | Registers desktop launcher and associates `roblox-studio://` browser URI handler. |
| `./bin/cutlery doctor` | Diagnostic command checking Wine, NTSync, Vulkan, Driver, and Prefix health. |
| `./bin/cutlery help` | Shows the command-line help summary. |

### Environment Variables
You can customize the runner behavior using environment variables:

- `WINEPREFIX`: Specify a custom Wine prefix location (default: `sandbox/prefix`).
- `WINE`: Custom Wine binary to execute (e.g. `wine-staging` or a custom proton runner).
- `WINE_STUDIO_DOCKING_FIX`: Set to `1` (default enabled) to bypass `_NET_WM_MOVERESIZE` during panel drags.

---

##  MCP Safety Guardrails

When working with Roblox Studio via AI Agents, the bundled rules in `plugins/antigravity-roblox/rules/AGENTS.md` enforce:

1. **Mandatory Studio Discovery**: Agents must call `list_roblox_studios` first to verify that a Studio instance and Place are active.
2. **DataModel State Verification**: Editing operations (`multi_edit`, `insert_asset`) are strictly gated to `datamodel_type: "Edit"`.
3. **Safe Luau Execution (`execute_luau`)**:
   - Always wrap code inside `pcall`.
   - Never return raw Roblox `Instance` references directly (return primitive strings/tables or `HttpService:JSONEncode()` to prevent JSON-RPC transport drops).
   - Prevent engine freezes by yielding (`task.wait()`) during iterative operations.

---

##  Acknowledgments

Cutlery stands on the shoulders of giants. We express our deepest gratitude to the open-source projects and communities that pioneered Roblox gaming and development on Linux:

- **[Vinegar](https://github.com/vinegarhq/vinegar)**: For pioneering modern Wine/DXVK runtime configurations, DLL override strategies, and FastFlags optimization for Roblox on Linux.
- **[Grapejuice](https://gitlab.com/brinkervii/grapejuice)**: For proving years ago that running Roblox Studio natively on Linux with Wine was possible.
- **[DXVK](https://github.com/doitsujin/dxvk)**: For providing high-performance Direct3D 11 to Vulkan translation and DirectComposition swapchains.
- **[WineHQ](https://www.winehq.org)**: For decades of relentless engineering making Windows binaries thrive on POSIX systems.

---

##  License
This project is licensed under the **GNU General Public License v3.0 (GPL-3.0)** - see the [LICENSE](LICENSE) file for details. Components of Wine and DXVK are governed by their respective LGPL-2.1 and zlib/libpng licenses.


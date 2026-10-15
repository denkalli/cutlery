---
name: roblox-studio-mcp
description: >-
  Best practices, safe workflows, and error recovery for interacting with Roblox Studio via the Roblox Studio MCP Server.
  Use this skill whenever interacting with Roblox Studio MCP tools (e.g. execute_luau, multi_edit, script_read, search_game_tree, inspect_instance, start_stop_play), preventing connection drops, return code 1 crashes, JSON-RPC transport errors, and studio desyncs.
---

# Roblox Studio MCP Best Practices & Operational Guide

This skill provides step-by-step procedures, safety guardrails, and error recovery protocols for interacting with Roblox Studio through the Roblox Studio MCP server.

---

## 1. Architecture & Core Workflow

Roblox Studio MCP communicates through a multi-tier bridge:
`Antigravity / AI Agent (MCP Client) <-> python mcp/bridge.py <-> wine StudioMCP.exe (Proxy) <-> Roblox Studio (Win32 Named Pipe)`

```
+-------------------+       +-----------------------+       +-----------------+       +-------------------+
|  AI Agent Client  | <---> |     mcp/bridge.py     | <---> |  StudioMCP.exe  | <---> |   Roblox Studio   |
| (Claude/Antigrav) |       | (Intercepts probes)   | (Wine)|   (MCP Proxy)   | (IPC) | (Active Place/IDE)|
+-------------------+       +-----------------------+       +-----------------+       +-------------------+
```

> **Configuration Best Practice**: In `mcp_config.json`, configure `Roblox_Studio` to launch `python3` with `mcp/bridge.py`. The bridge dynamically locates `StudioMCP.exe` in the Wine prefix, intercepts pre-initialization probes (`server/discover`), and maintains a pristine JSON-RPC stream.

### The 3 Golden Rules
1. **Always fetch `studio_id` first**: Never guess or hardcode a studio ID. Call `list_roblox_studios` before any tool call.
2. **Check and match `datamodel_type`**: Edit operations (`multi_edit`, `insert_asset`) require `"Edit"`. Runtime inspections require `"Client"` or `"Server"`. Use `get_studio_state` to verify.
3. **Sanitize Luau return values**: Never return raw Roblox `Instance` objects or circular tables from `execute_luau`. Always serialize to strings, primitives, or JSON.

---

## 2. Step-by-Step Tool Procedures

### A. Initializing Connection & Discovering Studio
Every task interacting with Roblox Studio must follow this setup sequence:

```
Step 1: Call `list_roblox_studios`
        - If result is empty `[]`:
          -> Inform the user: "Roblox Studio is not open or no Place is loaded. Please open Roblox Studio and open a place file."
          -> Stop and do not execute further MCP tools.
        - If result contains one or more studios:
          -> Select the target `studio_id` (confirm with user if multiple studios are open).

Step 2: Call `get_studio_state` with `studio_id`
        - Check current mode: `Edit`, `PlayClient`, `PlayServer`, etc.
        - Check available `datamodel_type`s (e.g. `["Edit"]` or `["Client", "Server"]`).
```

---

### B. Safe Luau Execution (`execute_luau`)

Improper Luau scripts are the **#1 cause of Studio freezing and MCP returning code 1 / EOF**.

#### Safety Checklist for `execute_luau`:
- [ ] **Always wrap in `pcall`**: Prevent uncaught script errors from breaking the response.
- [ ] **Avoid infinite loops**: Never write `while true do` without `task.wait()` or an explicit iteration cap.
- [ ] **Never return raw Instances**: Returning `game.Workspace.MyPart` causes JSON-RPC serialization failure. Return `part.Name`, `part:GetFullName()`, or property dictionaries.
- [ ] **Use JSON Encoding for complex tables**: Use `game:GetService("HttpService"):JSONEncode(data)` to return safe stringified payloads.

#### Recommended Luau Execution Template:
```lua
local success, result = pcall(function()
    local HttpService = game:GetService("HttpService")
    
    -- Your logic here:
    local target = workspace:FindFirstChild("SpawnLocation")
    if not target then
        return { success = false, message = "SpawnLocation not found" }
    end
    
    return {
        success = true,
        name = target.Name,
        position = { target.Position.X, target.Position.Y, target.Position.Z },
        anchored = target.Anchored
    }
end)

if not success then
    return { success = false, error = tostring(result) }
else
    return result
end
```

See [Luau Execution Guide](./references/luau-execution-guide.md) for advanced patterns and batch operations.

---

### C. Safe Script Reading & Editing (`script_read` & `multi_edit`)

Modifying scripts requires exact matching to prevent corrupted code.

#### 1. Reading Scripts (`script_read`):
- Path format must use dot-notation (e.g., `"game.ServerScriptService.GameManager"` or `"ServerScriptService.GameManager"`).
- Always read the script before attempting `multi_edit` to ensure exact character and whitespace matching.

#### 2. Editing Existing Scripts (`multi_edit`):
- `old_string` must match the target content **identically** (including leading tabs/spaces and newline format).
- `old_string` and `new_string` must be different.
- `datamodel_type` must be `"Edit"`.

#### 3. Creating New Scripts (`multi_edit`):
- Set `className`: `"Script"` (Server), `"LocalScript"` (Client), or `"ModuleScript"`.
- The first edit entry must have `old_string: ""` (empty string) to populate initial content.
- Example:
  ```json
  {
    "studio_id": "<ID>",
    "datamodel_type": "Edit",
    "file_path": "game.ServerScriptService.NewModule",
    "className": "ModuleScript",
    "edits": [
      {
        "old_string": "",
        "new_string": "local NewModule = {}\n\nfunction NewModule.init()\n    print('Initialized')\nend\n\nreturn NewModule"
      }
    ]
  }
  ```

---

### D. Safe Hierarchy Exploration (`search_game_tree` & `inspect_instance`)

Roblox places can contain hundreds of thousands of instances. Unbounded queries will blow up the JSON-RPC message buffer and disconnect the MCP server.

- **Always specify `path`**: Narrow down to `"Workspace"`, `"ReplicatedStorage"`, `"ServerStorage"`, or specific models.
- **Keep `max_depth` low**: Default is `3`. Do not set `max_depth > 5` unless specifically required and filtered.
- **Use filters**: Use `instance_type` (e.g., `"BaseScript"`, `"Model"`, `"Part"`) or `keywords` to limit results.
- **Use `inspect_instance` for deep property inspection**: Do not traverse the tree just to read properties; use `inspect_instance(path="Workspace.Model.Part")`.

---

## 3. Common Error Signatures & Remediation Runbook

| Error Signature | Root Cause | Immediate Fix |
| :--- | :--- | :--- |
| `expect initialized request, but received: server/discover` | MCP client sent pre-init discovery probe directly to `StudioMCP.exe`. | Ensure MCP client calls `mcp/bridge.py`, which intercepts discovery probes and replies locally. |
| `connection closed: calling "initialize": client is closing: EOF` | `mcp.bat` crashed or StudioMCP.exe exited immediately on launch. | Check `mcp.bat` syntax (e.g. `else` on newline) and verify `StudioMCP.exe` exists in `AppData\Local\Roblox\Versions\...`. |
| `Tool returned code 1 / Not responding` | Luau script threw an unhandled error, blocked main thread in an infinite loop, or returned an un-serializable Instance. | Kill/restart the call, wrap Luau in `pcall`, add `task.wait()` in loops, and return primitive data types or JSON strings. |
| `Studio instance not found` or `list_roblox_studios` returns `[]` | Roblox Studio is closed, in the startup splash screen, or place is not loaded. | Prompt user to open Roblox Studio and load their place file. |
| `datamodel_type not supported in current mode` | Tried to call an Edit-only tool during playtest, or vice-versa. | Call `get_studio_state`. If needed, use `start_stop_play` to stop or start play mode. |
| `multi_edit failed: old_string not found` | Whitespace, indentation, or line-ending mismatch. | Call `script_read` to fetch the exact lines, then copy the exact substring into `old_string`. |

For complete troubleshooting details, see [Troubleshooting Reference](./references/troubleshooting.md).

---

## 4. References & Documentation

- [Tool Reference](./references/tool-reference.md): Full breakdown of all 28 Roblox Studio MCP tools.
- [Luau Execution Guide](./references/luau-execution-guide.md): Safe coding patterns and serialization recipes.
- [Troubleshooting Reference](./references/troubleshooting.md): In-depth error resolution guide.

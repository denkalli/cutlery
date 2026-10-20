# Roblox Studio MCP Troubleshooting Reference

This guide covers error codes, crash scenarios, JSON-RPC transport issues, and remediation steps when working with the Roblox Studio MCP server.

---

## 1. Process & Connection Errors

### A. "expect initialized request, but received: ... server/discover"
- **Symptoms**: MCP connection closes immediately with EOF, or returns error code 1 during startup.
- **Cause**: The MCP client (e.g. Antigravity) probes the server with `server/discover` before sending `initialize`. `StudioMCP.exe` only accepts `initialize` as its very first packet.
- **Remediation**:
  1. In `mcp_config.json`, configure `Roblox_Studio` to run `python3` directly with `mcp/bridge.py`:
     ```json
     "Roblox_Studio": {
       "command": "python3",
       "args": ["/absolute/path/to/cutlery/mcp/bridge.py"]
     }
     ```
  2. The bridge intercepts `server/discover` probes locally, passes `initialize` safely to Wine, and dynamically locates `StudioMCP.exe` across Roblox version updates.

---

### B. "Studio not found" / `list_roblox_studios` returns `[]`
- **Symptoms**: `list_roblox_studios` returns an empty array.
- **Cause**:
  1. Roblox Studio is not running.
  2. Roblox Studio is on the initial start page (no Place / Experience opened).
  3. Studio crashed or became unresponsive.
- **Remediation**:
  1. Launch Roblox Studio.
  2. Open an existing `.rbxl` place file or create a new template place.
  3. Re-run `list_roblox_studios`.

---

### C. "connection closed: calling initialize: client is closing: EOF"
- **Symptoms**: Tool returns code 1 and communication terminates.
- **Cause**:
  1. Batch file syntax errors in `mcp.bat` (e.g., `else` on a newline instead of `) else (`).
  2. Missing Python in system PATH.
  3. Studio was closed while a request was pending.
- **Remediation**:
  1. Test batch execution manually: `cmd.exe /c "%LOCALAPPDATA%\Roblox\mcp.bat"`
  2. Confirm Python is accessible: `python --version`

---

## 2. Luau Execution & DataModel Errors

### A. Studio Freezes / MCP Call Hangs Forever
- **Cause**: Luau code executed via `execute_luau` ran an infinite loop without `task.wait()`, locking the main thread.
- **Remediation**:
  1. In Roblox Studio, press Shift+F5 or Stop playtest.
  2. Ensure all loops in `execute_luau` have yield guards:
     ```lua
     if count % 100 == 0 then task.wait() end
     ```

---

### B. JSON-RPC Serialization / Transport Error
- **Cause**: `execute_luau` returned a raw Roblox `Instance` (e.g. `workspace.Part`) or a table with cyclical references.
- **Remediation**:
  1. Extract only primitive data (strings, numbers, booleans, nested dictionaries of primitives).
  2. Serialize using `game:GetService("HttpService"):JSONEncode(result)`.

---

### C. "datamodel_type not supported in current mode"
- **Cause**: Calling an Edit-only tool (`multi_edit`, `insert_asset`) during Play/Run mode, or calling `execute_luau` with `datamodel_type: "Client"` when in Edit mode.
- **Remediation**:
  1. Check current state with `get_studio_state`.
  2. Use `start_stop_play(action: "Stop")` to return to Edit mode, or `"Play"`/`"Run"` to enter simulation.

---

### D. "multi_edit: old_string not found"
- **Cause**: Slight discrepancy in whitespace, indentation, or newline format (`\r\n` vs `\n`).
- **Remediation**:
  1. Call `script_read` to fetch the current script content.
  2. Copy the exact lines from the `script_read` output into `old_string`.

# Roblox Studio Integration Rules & Safety Protocols

When interacting with Roblox Studio via the Roblox Studio MCP Server:

1. **Mandatory Studio Discovery**:
   - Always call `list_roblox_studios` first to obtain the valid `studio_id`.
   - Never hardcode or guess `studio_id`. If `list_roblox_studios` returns an empty array `[]`, instruct the user to open Roblox Studio and load a place.

2. **DataModel State Verification**:
   - Query current state using `get_studio_state`.
   - Ensure editing operations (`multi_edit`, `insert_asset`) are performed exclusively when `datamodel_type` is `"Edit"`.

3. **Luau Execution Guardrails (`execute_luau`)**:
   - Wrap Luau code inside `pcall` to catch runtime errors cleanly.
   - Never return raw Roblox `Instance` objects directly. Extract primitive properties (numbers, strings, booleans, property tables) or use `HttpService:JSONEncode()`.
   - Prevent engine freezes by yielding (`task.wait()`) in repetitive loops.

4. **Script Container Property Limits**:
   - Be aware that Roblox Engine limits `LuaSourceContainer.Source` to `< 200,000` characters (~195 KB). Split codebases exceeding ~180 KB into `ModuleScript`s.

5. **Exact Script Modification (`multi_edit`)**:
   - Always run `script_read` before modifying scripts to match `old_string` character-for-character, including whitespace and line endings.
   - For new scripts, specify `className` and set `old_string: ""` in the first edit block.

6. **Search Tool Argument Precision**:
   - `script_search`: Requires `keywords` (comma-separated string for script name matching).
   - `script_grep`: Requires `query` (string or Luau pattern for script content search).

# Roblox Studio MCP Tool Reference

This document provides a comprehensive categorized reference of all 28 Roblox Studio MCP tools.

---

## 1. Studio Lifecycle & State

### `list_roblox_studios`
- **Purpose**: Lists all active Roblox Studio instances connected to the MCP proxy.
- **When to Use**: **Must be called first** in every conversation before invoking any tool requiring `studio_id`.
- **Arguments**: None.
- **Return Example**:
  ```json
  [
    {
      "id": "123456789",
      "name": "MyPlace (PlaceId: 123456)"
    }
  ]
  ```

---

### `get_studio_state`
- **Purpose**: Inspects the current state of Studio, including play state (`Edit`, `PlayClient`, `PlayServer`) and available `datamodel_type`s.
- **Arguments**:
  - `studio_id` (string, required)
- **When to Use**: Verify available datamodels before executing `multi_edit` or `execute_luau`.

---

### `start_stop_play`
- **Purpose**: Starts, pauses, or stops simulation/playtesting in Roblox Studio.
- **Arguments**:
  - `studio_id` (string, required)
  - `action` (string, required: `"Play"`, `"Run"`, `"Stop"`, `"Pause"`, `"Resume"`)

---

## 2. Hierarchy & Game Tree Inspection

### `search_game_tree`
- **Purpose**: Traverses the Roblox instance hierarchy and returns structured JSON nodes.
- **Arguments**:
  - `studio_id` (string, required)
  - `datamodel_type` (string, required: `"Edit"`, `"Client"`, `"Server"`)
  - `path` (string, optional: e.g. `"Workspace"`, `"ReplicatedStorage"`)
  - `instance_type` (string, optional: e.g. `"BaseScript"`, `"Part"`, `"Model"`)
  - `keywords` (string, optional: e.g. `"Door"`, `"PlayerSpawn"`)
  - `max_depth` (number, optional, default: `3`, max: `10`)
  - `head_limit` (number, optional, default: `200`)
- **Safety Tip**: Always provide `path` to avoid traversing the entire `game` tree.

---

### `inspect_instance`
- **Purpose**: Retrieves all readable properties, custom attributes, and children summary for an instance.
- **Arguments**:
  - `studio_id` (string, required)
  - `path` (string, required: dot notation e.g. `"Workspace.SpawnLocation"`)
- **Notes**: Case-insensitive; returns up to 20 matches if ambiguous.

---

## 3. Script Inspection & Editing

### `script_read`
- **Purpose**: Reads script contents with line numbers (`LINE_NUMBER->LINE_CONTENT`).
- **Arguments**:
  - `studio_id` (string, required)
  - `target_file` (string, required: e.g. `"game.ServerScriptService.MainModule"`)
  - `should_read_entire_file` (boolean, optional, default: `true`)
  - `start_line_one_indexed` (integer, optional)
  - `end_line_one_indexed_inclusive` (integer, optional)

---

### `multi_edit`
- **Purpose**: Applies one or more text replacements to an existing script or creates a new script.
- **Arguments**:
  - `studio_id` (string, required)
  - `datamodel_type` (string, required: `"Edit"`)
  - `file_path` (string, required: dot-notation)
  - `edits` (array of `{ old_string, new_string, replace_all? }`, required)
  - `className` (string, required for new scripts: `"Script"`, `"LocalScript"`, `"ModuleScript"`)
- **Rules**:
  - For new scripts, `old_string` in the first edit must be `""`.
  - For edits, `old_string` must match the source script exactly.

---

### `script_search`
- **Purpose**: Fuzzy search across script names in the game.
- **Arguments**:
  - `studio_id` (string, required)
  - `keywords` (string, required: comma-separated case-insensitive keywords)

---

### `script_grep`
- **Purpose**: Fast content search (string or Luau pattern) across all script bodies in the game.
- **Arguments**:
  - `studio_id` (string, required)
  - `query` (string, required: string or Luau pattern to search for)

---

## 4. Execution & Automated Logic

### `execute_luau`
- **Purpose**: Executes arbitrary Luau code live in Roblox Studio.
- **Arguments**:
  - `studio_id` (string, required)
  - `datamodel_type` (string, required: `"Edit"`, `"Client"`, `"Server"`)
  - `code` (string, required)
- **Safety Rules**:
  - Never return raw `Instance` references.
  - Wrap in `pcall`.
  - Yield with `task.wait()` in loops.

---

### `run_as_job`
- **Purpose**: Runs a longer asynchronous job in Roblox Studio and monitors completion.

---

## 5. Asset Insertion & AI Generation

### `insert_asset` & `search_asset`
- **Purpose**: Search the Roblox Creator Store for models, decals, audio, and meshes, and insert them into the active place.

### `generate_material`, `generate_mesh`, `generate_procedural_model`
- **Purpose**: Generative AI tools to build materials, 3D meshes, and procedural assets directly in Studio.

---

## 6. Diagnostics, Console & Visuals

### `get_console_output`
- **Purpose**: Fetches output/error logs from the Roblox Studio output window.

### `screen_capture`
- **Purpose**: Takes a screenshot of the active Roblox Studio viewport.

### `user_keyboard_input` & `user_mouse_input`
- **Purpose**: Simulates user inputs inside Studio for interactive testing.

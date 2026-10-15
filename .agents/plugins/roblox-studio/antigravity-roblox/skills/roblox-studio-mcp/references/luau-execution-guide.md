# Luau Execution Guide for Roblox Studio MCP

When running Luau code inside Roblox Studio via `execute_luau`, the execution environment is live inside the Studio process. If code is written improperly, it can freeze Roblox Studio, crash the JSON-RPC bridge, or cause StudioMCP to disconnect with exit code 1.

---

## 1. Golden Rules of `execute_luau`

### 1. Never Return Raw Roblox `Instance` Objects
Roblox `Instance` objects (e.g. `workspace.Baseplate`, `game.Players`, `script`) cannot be serialized across the JSON-RPC boundary directly. Returning raw instances will trigger serialization errors or return empty `{}` objects.

#### ❌ Dangerous (May Crash or Fail Serialization):
```lua
-- WRONG:
return workspace.SpawnLocation
```

#### ✅ Safe (Extract Primitives or Dictionaries):
```lua
-- CORRECT:
local part = workspace:FindFirstChild("SpawnLocation")
if not part then return nil end

return {
    name = part.Name,
    className = part.ClassName,
    position = { x = part.Position.X, y = part.Position.Y, z = part.Position.Z },
    size = { x = part.Size.X, y = part.Size.Y, z = part.Size.Z },
    anchored = part.Anchored,
    canCollide = part.CanCollide
}
```

---

### 2. Always Wrap Operations in `pcall`
Uncaught exceptions in Studio can cause unpredictable states. Wrapping in `pcall` guarantees a clean JSON response describing the failure.

```lua
local success, result = pcall(function()
    -- Your execution logic
    local ReplicatedStorage = game:GetService("ReplicatedStorage")
    local config = ReplicatedStorage:FindFirstChild("GameConfig")
    if not config then
        error("GameConfig not found in ReplicatedStorage")
    end
    return { found = true, configType = config.ClassName }
end)

if not success then
    return { success = false, error = tostring(result) }
else
    return { success = true, data = result }
end
```

---

### 3. Prevent Freezing with Yielding and Iteration Caps
Roblox Studio runs Luau on its main task scheduler. A tight loop without yields (`task.wait()`) will lock Studio, causing Antigravity's MCP client to timeout.

#### ❌ Dangerous Loop:
```lua
-- WRONG: Will lock Studio if hierarchy is deep
for _, desc in ipairs(workspace:GetDescendants()) do
    -- heavy logic
end
```

#### ✅ Safe Loop with Yielding and Batching:
```lua
local count = 0
local results = {}
local descendants = workspace:GetDescendants()

for i, desc in ipairs(descendants) do
    if desc:IsA("BasePart") and desc.Anchored == false then
        table.insert(results, desc:GetFullName())
    end
    
    count = count + 1
    if count % 200 == 0 then
        task.wait() -- Yield to allow Studio to breathe
    end
end

return results
```

---

### 4. Large Data / Complex Tables: Use `HttpService:JSONEncode`
When returning complex structures or lists, stringifying via `HttpService:JSONEncode()` avoids cyclic reference errors and guarantees valid JSON transmission.

```lua
local HttpService = game:GetService("HttpService")

local report = {
    totalParts = 0,
    scripts = {},
    warnings = {}
}

for _, obj in ipairs(workspace:GetChildren()) do
    if obj:IsA("BasePart") then
        report.totalParts += 1
    end
end

return HttpService:JSONEncode(report)
```

---

### 5. Roblox Engine Script Length Limit (200,000 chars / ~195 KB)
The Roblox Engine enforces a strict property limit on `LuaSourceContainer.Source` (`Script`, `LocalScript`, `ModuleScript`):
- **Max length**: `< 200,000` characters (~195 KB).
- Exceeding this limit throws: `Unable to assign property Source. Provided string length (...) is greater than or equal to max length (200000)`.
- **Best Practice**: If code or data tables approach ~180 KB, modularize and split them across multiple `ModuleScript`s. (Note: The JSON-RPC transport layer itself supports 1MB+ payloads, but the Roblox script container is capped at 200k chars).

---

## 2. Common Patterns & Recipes

### A. Creating Instances Safely in Edit Mode
```lua
local success, result = pcall(function()
    local folder = workspace:FindFirstChild("GeneratedMap")
    if not folder then
        folder = Instance.new("Folder")
        folder.Name = "GeneratedMap"
        folder.Parent = workspace
    end

    local part = Instance.new("Part")
    part.Name = "Platform"
    part.Size = Vector3.new(20, 2, 20)
    part.Position = Vector3.new(0, 5, 0)
    part.Anchored = true
    part.Material = Enum.Material.SmoothPlastic
    part.Color = Color3.fromRGB(0, 160, 255)
    part.Parent = folder

    return { created = true, path = part:GetFullName() }
end)

return result
```

### B. Safe Property Inspection
```lua
local function inspectProperties(instance)
    local HttpService = game:GetService("HttpService")
    local data = {
        name = instance.Name,
        className = instance.ClassName,
        parent = instance.Parent and instance.Parent.Name or "nil",
        attributes = instance:GetAttributes(),
        tags = game:GetService("CollectionService"):GetTags(instance)
    }
    return HttpService:JSONEncode(data)
end

return inspectProperties(workspace.Baseplate)
```

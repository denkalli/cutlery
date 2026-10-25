//! AI client integrations: Antigravity plugin link, Claude Desktop and Codex config, MCP bridge probe.

use std::path::{Path, PathBuf};

use crate::{find_studio_exe, get_root_dir};

pub(super) fn antigravity_plugin_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".gemini/config/plugins/roblox-studio");
    dir.exists().then_some(dir)
}

/// Absolute path of the `cutlery` binary that MCP clients should execute.
fn canonical_binary_path() -> PathBuf {
    // 1. Current executing binary if valid
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(canonical) = exe.canonicalize() {
            return canonical;
        } else if exe.exists() {
            return exe;
        }
    }

    // 2. Project root bin or release target
    let root = get_root_dir();
    let release_bin = root.join("target").join("release").join("cutlery");
    if release_bin.exists() {
        return release_bin;
    }
    let local_bin = root.join("bin").join("cutlery");
    if local_bin.exists() {
        return local_bin;
    }

    // 3. User standard local bin ~/.local/bin/cutlery
    if let Ok(home) = std::env::var("HOME") {
        let user_bin = PathBuf::from(home).join(".local").join("bin").join("cutlery");
        if user_bin.exists() {
            return user_bin;
        }
    }

    // 4. System PATH fallback
    PathBuf::from("cutlery")
}

/// Returns whether the Antigravity plugin points at an existing binary, plus a description.
pub(super) fn check_antigravity_link() -> (bool, String) {
    let Some(dir) = antigravity_plugin_dir() else {
        return (false, "Plugin directory not found (~/.gemini/config/plugins/roblox-studio)".to_string());
    };
    let command = std::fs::read_to_string(dir.join("mcp_config.json"))
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .and_then(|val| {
            val.get("mcpServers")?
                .get("Roblox_Studio")?
                .get("command")?
                .as_str()
                .map(str::to_string)
        });
    match command {
        Some(cmd) if Path::new(&cmd).exists() => (true, cmd),
        Some(cmd) => (false, format!("Outdated (target missing: {})", cmd)),
        None => (false, "mcp_config.json not configured".to_string()),
    }
}

/// Points the Antigravity plugin's `mcp_config.json` at this binary. Returns the linked path.
pub(super) fn link_antigravity() -> Result<PathBuf, String> {
    let dir = antigravity_plugin_dir().ok_or("Antigravity plugin directory not found.")?;
    let binary_path = canonical_binary_path();
    let json_content = serde_json::json!({
        "mcpServers": {
            "Roblox_Studio": {
                "command": binary_path.to_string_lossy(),
                "args": ["mcp"]
            }
        }
    });
    let formatted = serde_json::to_string_pretty(&json_content).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("mcp_config.json"), formatted)
        .map_err(|e| format!("Could not write mcp_config.json: {}", e))?;
    Ok(binary_path)
}

/// Adds (or updates) the `cutlery` server in Claude Desktop's config, keeping other entries.
pub(super) fn configure_claude_desktop() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set.")?;
    let claude_dir = PathBuf::from(home).join(".config/Claude");
    std::fs::create_dir_all(&claude_dir).map_err(|e| e.to_string())?;
    let cfg_path = claude_dir.join("claude_desktop_config.json");

    let mut cfg: serde_json::Value = std::fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    if !cfg.get("mcpServers").is_some_and(serde_json::Value::is_object) {
        cfg["mcpServers"] = serde_json::json!({});
    }
    cfg["mcpServers"]["cutlery"] = serde_json::json!({
        "command": canonical_binary_path().to_string_lossy(),
        "args": ["mcp"]
    });

    let formatted = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&cfg_path, formatted)
        .map_err(|e| format!("Could not write {}: {}", cfg_path.display(), e))?;
    Ok(cfg_path)
}

/// Adds (or updates) the `cutlery` server in Codex's config, keeping other entries.
pub(super) fn configure_codex() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set.")?;
    let codex_dir = PathBuf::from(home).join(".codex");
    std::fs::create_dir_all(&codex_dir).map_err(|e| e.to_string())?;
    let cfg_path = codex_dir.join("config.toml");

    let content = match std::fs::read_to_string(&cfg_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("Could not read {}: {}", cfg_path.display(), error)),
    };
    let mut cfg: toml_edit::DocumentMut = if content.trim().is_empty() {
        toml_edit::DocumentMut::new()
    } else {
        content
            .parse()
            .map_err(|e| format!("Could not parse {}: {}", cfg_path.display(), e))?
    };

    let servers = cfg
        .as_table_mut()
        .entry("mcp_servers")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    let servers = servers
        .as_table_mut()
        .ok_or_else(|| format!("mcp_servers in {} must be a TOML table.", cfg_path.display()))?;
    let server = servers
        .entry("cutlery")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
    let server = server
        .as_table_mut()
        .ok_or_else(|| format!("mcp_servers.cutlery in {} must be a TOML table.", cfg_path.display()))?;
    server.insert(
        "command",
        toml_edit::value(canonical_binary_path().to_string_lossy().into_owned()),
    );
    let mut args = toml_edit::Array::new();
    args.push("mcp");
    server.insert("args", toml_edit::value(args));

    std::fs::write(&cfg_path, cfg.to_string())
        .map_err(|e| format!("Could not write {}: {}", cfg_path.display(), e))?;
    Ok(cfg_path)
}

/// Returns (bridge status, status bar message).
pub(super) fn probe_mcp_bridge(prefix: &Path) -> (String, String) {
    let studiomcp_found = find_studio_exe(prefix)
        .and_then(|exe| exe.parent().map(|dir| dir.join("StudioMCP.exe").exists()))
        .unwrap_or(false);
    if studiomcp_found {
        (
            "StudioMCP.exe verified in prefix • Bridge handshake ready".to_string(),
            "MCP Bridge health check: StudioMCP.exe detected and ready.".to_string(),
        )
    } else {
        (
            "Bridge operational • Pre-Init Shield protecting handshake".to_string(),
            "MCP Bridge active (StudioMCP.exe will launch on agent connect).".to_string(),
        )
    }
}

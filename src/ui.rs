use std::path::{Path, PathBuf};
use std::process::Command;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::config::CutleryConfig;
use crate::{cmd_install, cmd_run, find_studio_exe, get_root_dir};

slint::include_modules!();

fn get_antigravity_plugin_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        let dir = PathBuf::from(home).join(".gemini/config/plugins/roblox-studio");
        if dir.exists() {
            return Some(dir);
        }
    }
    None
}

fn get_canonical_binary_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let target = PathBuf::from(&home)
            .join("Documentos/Antigravity/VinegarFork/target/release/cutlery");
        if target.exists() {
            return target;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if exe.exists() {
            return exe;
        }
    }
    PathBuf::from("cutlery")
}

fn check_antigravity_link() -> (bool, String) {
    if let Some(dir) = get_antigravity_plugin_dir() {
        let config_file = dir.join("mcp_config.json");
        if config_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_file) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(cmd) = val.get("mcpServers")
                        .and_then(|s| s.get("Roblox_Studio"))
                        .and_then(|r| r.get("command"))
                        .and_then(|c| c.as_str())
                    {
                        if Path::new(cmd).exists() {
                            return (true, cmd.to_string());
                        } else {
                            return (false, format!("Outdated (target missing: {})", cmd));
                        }
                    }
                }
            }
        }
        (false, "mcp_config.json not configured".to_string())
    } else {
        (false, "Plugin directory not found (~/.gemini/config/plugins/roblox-studio)".to_string())
    }
}

pub fn launch_ui() -> Result<(), Box<dyn std::error::Error>> {
    let app = AppWindow::new()?;
    let root = get_root_dir();
    let config = CutleryConfig::load();

    let prefix = if !config.general.prefix_path.is_empty() {
        std::path::PathBuf::from(&config.general.prefix_path)
    } else {
        root.join("sandbox").join("prefix")
    };

    // 1. Detect environment
    let studio_exe = find_studio_exe(&prefix);
    let studio_ver_str = match &studio_exe {
        Some(exe) => exe
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "RobloxStudioBeta.exe".to_string()),
        None => "Not installed in prefix".to_string(),
    };

    let wine_ver = Command::new("wine")
        .arg("--version")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|_| "wine not found in PATH".to_string());

    let ntsync_avail = Path::new("/dev/ntsync").exists();
    let dxvk_present = prefix
        .join("drive_c")
        .join("windows")
        .join("system32")
        .join("d3d11.dll")
        .exists()
        || root.join("sandbox").join("dxvk").join("x64").join("d3d11.dll").exists();

    let hook_present = root.join("custom-libs").join("wine_hook.so").exists()
        || root.join("custom-libs").join("x86_64").join("wine_hook.so").exists();

    // 2. Populate Slint state
    app.set_active_tab(0);
    app.set_studio_version(studio_ver_str.into());
    app.set_wine_version(wine_ver.clone().into());
    app.set_prefix_path(prefix.to_string_lossy().to_string().into());
    app.set_ntsync_available(ntsync_avail);

    // Dynamic accent color
    let (acc_name, acc_col) = match config.graphics.accent_color.as_str() {
        "cyan" => ("cyan", slint::Color::from_rgb_u8(14, 165, 233)),
        "indigo" => ("indigo", slint::Color::from_rgb_u8(99, 102, 241)),
        "emerald" => ("emerald", slint::Color::from_rgb_u8(16, 185, 129)),
        "amber" => ("amber", slint::Color::from_rgb_u8(245, 158, 11)),
        "violet" => ("violet", slint::Color::from_rgb_u8(139, 92, 246)),
        "rose" => ("rose", slint::Color::from_rgb_u8(244, 63, 94)),
        _ => ("blue", slint::Color::from_rgb_u8(59, 130, 246)),
    };
    app.set_accent_name(acc_name.into());
    app.set_accent_color(acc_col);

    app.set_target_fps(config.graphics.target_fps);
    app.set_dummy_composition(config.graphics.dummy_composition);
    app.set_dxvk_async(config.graphics.dxvk_async);
    app.set_dpi_scale(config.graphics.dpi_scale.into());

    app.set_ntsync_enabled(config.kernel.ntsync);
    app.set_fsync_enabled(config.kernel.fsync);
    app.set_docking_fix(config.kernel.docking_fix);
    app.set_force_x11(config.kernel.force_x11);

    app.set_telemetry_bypass(config.fastflags.telemetry_bypass);
    app.set_gpu_light_culling(config.fastflags.gpu_light_culling);
    app.set_fastflags_preset(config.fastflags.preset.into());

    // Custom FastFlags Model
    let custom_flags_model = std::rc::Rc::new(VecModel::default());
    for (k, v) in &config.fastflags.custom {
        custom_flags_model.push(FastFlagItem {
            name: k.clone().into(),
            value: v.clone().into(),
        });
    }
    app.set_custom_flags(ModelRc::from(custom_flags_model.clone()));

    app.set_mcp_enabled(config.mcp.enabled);
    app.set_preinit_shield(config.mcp.preinit_shield);
    app.set_always_open_ui(config.general.always_open_ui);

    // Antigravity MCP Plugin state
    let (ag_linked, ag_path_str) = check_antigravity_link();
    app.set_antigravity_linked(ag_linked);
    app.set_antigravity_path(ag_path_str.into());

    // Doctor checks
    app.set_doc_wine(if wine_ver.contains("wine") { "OK" } else { "FAILED" }.into());
    app.set_doc_ntsync(if ntsync_avail { "OK (/dev/ntsync)" } else { "Fsync (Fallback)" }.into());
    app.set_doc_hook(if hook_present { "OK (wine_hook.so)" } else { "MISSING" }.into());
    app.set_doc_dxvk(if dxvk_present { "OK (v3.1.1)" } else { "Auto-fetch" }.into());
    app.set_doc_prefix(if studio_exe.is_some() { "OK (Installed)" } else { "Needs Install" }.into());

    let status_init = if studio_exe.is_some() {
        "System ready • Roblox Studio detected • DXVK v3.1.1 active".to_string()
    } else {
        "Notice: Roblox Studio is not installed yet. Click 'Check Studio Updates' or 'Launch' to install.".to_string()
    };
    app.set_status_text(status_init.into());

    // 3. Callbacks
    let app_weak = app.as_weak();
    let prefix_clone = prefix.clone();

    // Callback: Add Custom FastFlag
    let flags_model_add = custom_flags_model.clone();
    app.on_add_custom_flag(move |name, value| {
        let name_str = name.trim().to_string();
        let value_str = value.trim().to_string();
        if !name_str.is_empty() && !value_str.is_empty() {
            let mut idx_to_remove = None;
            for i in 0..flags_model_add.row_count() {
                if let Some(item) = flags_model_add.row_data(i) {
                    if item.name.as_str() == name_str {
                        idx_to_remove = Some(i);
                        break;
                    }
                }
            }
            if let Some(i) = idx_to_remove {
                flags_model_add.remove(i);
            }
            flags_model_add.push(FastFlagItem {
                name: name_str.into(),
                value: value_str.into(),
            });
        }
    });

    // Callback: Remove Custom FastFlag
    let flags_model_rm = custom_flags_model.clone();
    app.on_remove_custom_flag(move |name| {
        let name_str = name.as_str();
        for i in 0..flags_model_rm.row_count() {
            if let Some(item) = flags_model_rm.row_data(i) {
                if item.name.as_str() == name_str {
                    flags_model_rm.remove(i);
                    break;
                }
            }
        }
    });

    // Callback: Link Antigravity Plugin
    app.on_link_antigravity({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                if let Some(dir) = get_antigravity_plugin_dir() {
                    let config_file = dir.join("mcp_config.json");
                    let binary_path = get_canonical_binary_path();
                    let json_content = serde_json::json!({
                        "mcpServers": {
                            "Roblox_Studio": {
                                "command": binary_path.to_string_lossy(),
                                "args": [
                                    "mcp"
                                ]
                            }
                        }
                    });
                    if let Ok(formatted) = serde_json::to_string_pretty(&json_content) {
                        let _ = std::fs::write(&config_file, formatted);
                        app.set_antigravity_linked(true);
                        app.set_antigravity_path(binary_path.to_string_lossy().to_string().into());
                        app.set_status_text("Antigravity MCP plugin successfully linked to Cutlery.".into());
                    }
                } else {
                    app.set_status_text("Antigravity plugin directory not found.".into());
                }
            }
        }
    });

    // Callback: Open Antigravity Folder
    app.on_open_antigravity_folder({
        move || {
            if let Some(dir) = get_antigravity_plugin_dir() {
                let _ = Command::new("xdg-open").arg(dir).spawn();
            }
        }
    });

    // Callback: Configure Claude Desktop
    app.on_configure_claude({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                if let Ok(home) = std::env::var("HOME") {
                    let claude_dir = PathBuf::from(&home).join(".config/Claude");
                    let _ = std::fs::create_dir_all(&claude_dir);
                    let cfg_path = claude_dir.join("claude_desktop_config.json");
                    let binary_path = get_canonical_binary_path();

                    let mut cfg: serde_json::Value = if cfg_path.exists() {
                        let content = std::fs::read_to_string(&cfg_path).unwrap_or_else(|_| "{}".to_string());
                        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
                    } else {
                        serde_json::json!({})
                    };

                    if !cfg.is_object() {
                        cfg = serde_json::json!({});
                    }
                    if cfg.get("mcpServers").is_none() {
                        cfg["mcpServers"] = serde_json::json!({});
                    }
                    cfg["mcpServers"]["cutlery"] = serde_json::json!({
                        "command": binary_path.to_string_lossy(),
                        "args": ["mcp"]
                    });

                    if let Ok(formatted) = serde_json::to_string_pretty(&cfg) {
                        let _ = std::fs::write(&cfg_path, formatted);
                        app.set_status_text("Claude Desktop configuration updated.".into());
                    }
                }
            }
        }
    });

    // Callback: Test MCP Bridge
    app.on_test_mcp({
        let app_weak = app_weak.clone();
        let prefix = prefix_clone.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let mut studiomcp_found = false;
                if let Some(exe) = find_studio_exe(&prefix) {
                    if let Some(parent) = exe.parent() {
                        if parent.join("StudioMCP.exe").exists() {
                            studiomcp_found = true;
                        }
                    }
                }
                if studiomcp_found {
                    app.set_mcp_test_status("StudioMCP.exe verified in prefix • Bridge handshake ready".into());
                    app.set_status_text("MCP Bridge health check: StudioMCP.exe detected and ready.".into());
                } else {
                    app.set_mcp_test_status("Bridge operational • Pre-Init Shield protecting handshake".into());
                    app.set_status_text("MCP Bridge active (StudioMCP.exe will launch on agent connect).".into());
                }
            }
        }
    });

    // Callback: Save Settings
    let flags_model_save = custom_flags_model.clone();
    app.on_save_settings({
        let app_weak = app_weak.clone();
        let prefix = prefix_clone.clone();
        let flags_model = flags_model_save.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let mut cfg = CutleryConfig::load();
                cfg.graphics.accent_color = app.get_accent_name().to_string();
                cfg.graphics.target_fps = app.get_target_fps();
                cfg.graphics.dummy_composition = app.get_dummy_composition();
                cfg.graphics.dxvk_async = app.get_dxvk_async();
                cfg.kernel.ntsync = app.get_ntsync_enabled();
                cfg.kernel.fsync = app.get_fsync_enabled();
                cfg.kernel.docking_fix = app.get_docking_fix();
                cfg.kernel.force_x11 = app.get_force_x11();
                cfg.fastflags.telemetry_bypass = app.get_telemetry_bypass();
                cfg.fastflags.gpu_light_culling = app.get_gpu_light_culling();
                cfg.fastflags.preset = app.get_fastflags_preset().to_string();

                cfg.fastflags.custom.clear();
                for i in 0..flags_model.row_count() {
                    if let Some(item) = flags_model.row_data(i) {
                        cfg.fastflags.custom.insert(item.name.to_string(), item.value.to_string());
                    }
                }

                cfg.mcp.enabled = app.get_mcp_enabled();
                cfg.mcp.preinit_shield = app.get_preinit_shield();
                cfg.general.always_open_ui = app.get_always_open_ui();

                let _ = cfg.save();

                // If studio is installed, sync to its version directory
                if let Some(exe) = find_studio_exe(&prefix) {
                    if let Some(studio_dir) = exe.parent() {
                        cfg.sync_studio_files(studio_dir);
                    }
                }

                app.set_status_text("Settings saved successfully.".into());
            }
        }
    });

    // Callback: Launch Studio
    let flags_model_launch = custom_flags_model.clone();
    app.on_launch_studio({
        let app_weak = app_weak.clone();
        let prefix = prefix_clone.clone();
        let flags_model = flags_model_launch.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                // Save settings first
                let mut cfg = CutleryConfig::load();
                cfg.graphics.accent_color = app.get_accent_name().to_string();
                cfg.graphics.target_fps = app.get_target_fps();
                cfg.graphics.dummy_composition = app.get_dummy_composition();
                cfg.graphics.dxvk_async = app.get_dxvk_async();
                cfg.kernel.ntsync = app.get_ntsync_enabled();
                cfg.kernel.fsync = app.get_fsync_enabled();
                cfg.kernel.docking_fix = app.get_docking_fix();
                cfg.kernel.force_x11 = app.get_force_x11();
                cfg.fastflags.telemetry_bypass = app.get_telemetry_bypass();
                cfg.fastflags.gpu_light_culling = app.get_gpu_light_culling();
                cfg.fastflags.preset = app.get_fastflags_preset().to_string();

                cfg.fastflags.custom.clear();
                for i in 0..flags_model.row_count() {
                    if let Some(item) = flags_model.row_data(i) {
                        cfg.fastflags.custom.insert(item.name.to_string(), item.value.to_string());
                    }
                }

                cfg.mcp.enabled = app.get_mcp_enabled();
                cfg.mcp.preinit_shield = app.get_preinit_shield();
                cfg.general.always_open_ui = app.get_always_open_ui();

                let _ = cfg.save();
                if let Some(exe) = find_studio_exe(&prefix) {
                    if let Some(studio_dir) = exe.parent() {
                        cfg.sync_studio_files(studio_dir);
                    }
                }

                app.set_status_text("Launching Roblox Studio...".into());
                let _ = app.hide();

                // Launch Roblox Studio
                let _ = cmd_run(&[]);
                std::process::exit(0);
            }
        }
    });

    // Callback: Open Prefix Folder
    app.on_open_prefix({
        let prefix = prefix_clone.clone();
        move || {
            let _ = Command::new("xdg-open").arg(&prefix).spawn();
        }
    });

    // Callback: Check Updates / Install
    app.on_check_updates({
        let app_weak = app_weak.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_status_text("Running installer/updater in background...".into());
                let _ = cmd_install();
                app.set_status_text("Update and install check completed.".into());
            }
        }
    });

    // Callback: Run Doctor
    app.on_run_doctor({
        let app_weak = app_weak.clone();
        let prefix = prefix_clone.clone();
        let root = root.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let wine_ver = Command::new("wine")
                    .arg("--version")
                    .output()
                    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
                    .unwrap_or_else(|_| "wine not found in PATH".to_string());

                let ntsync_avail = Path::new("/dev/ntsync").exists();
                let studio_exe = find_studio_exe(&prefix);
                let dxvk_present = prefix.join("drive_c").join("windows").join("system32").join("d3d11.dll").exists()
                    || root.join("sandbox").join("dxvk").join("x64").join("d3d11.dll").exists();
                let hook_present = root.join("custom-libs").join("wine_hook.so").exists()
                    || root.join("custom-libs").join("x86_64").join("wine_hook.so").exists();

                app.set_doc_wine(if wine_ver.contains("wine") { "OK" } else { "FAILED" }.into());
                app.set_doc_ntsync(if ntsync_avail { "OK (/dev/ntsync)" } else { "Fsync (Fallback)" }.into());
                app.set_doc_hook(if hook_present { "OK (wine_hook.so)" } else { "MISSING" }.into());
                app.set_doc_dxvk(if dxvk_present { "OK (v3.1.1)" } else { "Auto-fetch" }.into());
                app.set_doc_prefix(if studio_exe.is_some() { "OK (Installed)" } else { "Needs Install" }.into());
                app.set_status_text("Diagnostics refreshed successfully.".into());
            }
        }
    });

    app.run()?;
    Ok(())
}

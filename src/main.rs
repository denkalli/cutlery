use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

fn get_root_dir() -> PathBuf {
    // 1. If CUTLERY_ROOT env var is set, use it
    if let Ok(root) = env::var("CUTLERY_ROOT") {
        return PathBuf::from(root);
    }

    // 2. Derive from executable directory
    if let Ok(exe_path) = env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            // Check if exe is in bin/ or target/release/
            if parent.ends_with("bin") || parent.ends_with("release") || parent.ends_with("debug") {
                if let Some(grandparent) = parent.parent() {
                    if parent.ends_with("release") || parent.ends_with("debug") {
                        if let Some(target_parent) = grandparent.parent() {
                            if target_parent.join("Cargo.toml").exists()
                                || target_parent.join("data").exists()
                                || target_parent.join("sandbox").exists()
                            {
                                return target_parent.to_path_buf();
                            }
                        }
                    }
                    if grandparent.join("Cargo.toml").exists()
                        || grandparent.join("data").exists()
                        || grandparent.join("sandbox").exists()
                    {
                        return grandparent.to_path_buf();
                    }
                }
            }
            if parent.join("Cargo.toml").exists()
                || parent.join("data").exists()
                || parent.join("sandbox").exists()
            {
                return parent.to_path_buf();
            }
        }
    }

    // 3. Fallback to standard user XDG data directory ~/.local/share/cutlery
    if let Ok(home) = env::var("HOME") {
        let user_share = PathBuf::from(home).join(".local").join("share").join("cutlery");
        if user_share.exists() {
            return user_share;
        }
    }

    // 4. Fallback to system data directory /usr/share/cutlery
    let sys_share = PathBuf::from("/usr/share/cutlery");
    if sys_share.exists() {
        return sys_share;
    }

    // Default safe fallback: ~/.local/share/cutlery
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local").join("share").join("cutlery")
}

fn find_studio_exe(prefix: &Path) -> Option<PathBuf> {
    let drive_c = prefix.join("drive_c");
    if !drive_c.exists() {
        return None;
    }

    // Search common version directories
    let versions_dir = drive_c.join("users");
    if let Ok(users) = fs::read_dir(&versions_dir) {
        for user in users.flatten() {
            let studio_versions = user
                .path()
                .join("AppData")
                .join("Local")
                .join("Roblox")
                .join("Versions");
            if let Ok(vers) = fs::read_dir(&studio_versions) {
                for ver in vers.flatten() {
                    let exe = ver.path().join("RobloxStudioBeta.exe");
                    if exe.exists() {
                        return Some(exe);
                    }
                }
            }
        }
    }
    None
}

const DXVK_SHA256: &str = "40565b4a724aadc4433fa4e010b4b23916d9b1f1baeee64e17186db94f54e608";

fn escape_desktop_exec(arg: &str) -> String {
    let mut escaped = String::with_capacity(arg.len() + 16);
    for c in arg.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '$' => escaped.push_str("\\$"),
            '%' => escaped.push_str("%%"),
            _ => escaped.push(c),
        }
    }
    escaped
}

fn ensure_dxvk(sandbox: &Path, data_dir: &Path) -> PathBuf {
    let sandbox_dxvk = sandbox.join("dxvk").join("x64");
    let data_dxvk = data_dir.join("dxvk").join("x64");

    if sandbox_dxvk.join("d3d11.dll").exists() {
        return sandbox_dxvk;
    }
    if data_dxvk.join("d3d11.dll").exists() {
        return data_dxvk;
    }

    println!("[Cutlery] DXVK not found locally. Downloading DXVK v3.1.1 (with dummy composition swapchain support)...");
    let dxvk_root = sandbox.join("dxvk");
    let _ = fs::create_dir_all(&dxvk_root);
    let tarball = sandbox.join("dxvk-3.1.1.tar.gz");
    let url = "https://github.com/doitsujin/dxvk/releases/download/v3.1.1/dxvk-3.1.1.tar.gz";

    let curl_status = Command::new("curl")
        .args(["-fSL", url, "-o", &tarball.to_string_lossy()])
        .status();

    if let Ok(status) = curl_status {
        if status.success() {
            let checksum_check = Command::new("sh")
                .args([
                    "-c",
                    &format!("echo \"{}  {}\" | sha256sum -c --status", DXVK_SHA256, tarball.to_string_lossy())
                ])
                .status();

            match checksum_check {
                Ok(chk) if chk.success() => {
                    let _ = Command::new("tar")
                        .args([
                            "-xzf",
                            &tarball.to_string_lossy(),
                            "-C",
                            &dxvk_root.to_string_lossy(),
                            "--strip-components=1",
                        ])
                        .status();
                    let _ = fs::remove_file(&tarball);
                    if sandbox_dxvk.join("d3d11.dll").exists() {
                        println!("[Cutlery] DXVK v3.1.1 downloaded, verified (SHA-256), and extracted successfully.");
                    }
                }
                _ => {
                    eprintln!("[Cutlery] ERROR: DXVK tarball SHA-256 integrity check failed! Aborting extraction.");
                    let _ = fs::remove_file(&tarball);
                }
            }
        }
    }

    sandbox_dxvk
}

fn cmd_run(args: &[String]) -> Result<ExitStatus, std::io::Error> {
    let root = get_root_dir();
    let data_dir = root.join("data");
    let sandbox = root.join("sandbox");
    let prefix = sandbox.join("prefix");
    let custom_libs = root.join("custom-libs");

    println!("========================================================");
    println!("  Cutlery");
    println!("  Roblox Studio Launcher for Linux with Native Docking & AI MCP");
    println!("========================================================");

    let studio_exe = match find_studio_exe(&prefix) {
        Some(exe) => exe,
        None => {
            eprintln!("[Cutlery] Error: RobloxStudioBeta.exe not found in '{}'", prefix.display());
            eprintln!("[Cutlery] Please run 'cutlery install' first.");
            std::process::exit(1);
        }
    };

    let studio_dir = studio_exe.parent().unwrap();

    // 1. Sync AppSettings.xml
    let app_settings = studio_dir.join("AppSettings.xml");
    if !app_settings.exists() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Settings>\n  <ContentFolder>content</ContentFolder>\n  <BaseUrl>https://www.roblox.com</BaseUrl>\n</Settings>\n";
        let _ = fs::write(&app_settings, xml);
    }

    // 2. Sync DXVK dlls and config (auto-downloads DXVK v3.1.1 if missing on fresh clones)
    let dxvk_x64 = ensure_dxvk(&sandbox, &data_dir);
    if dxvk_x64.join("d3d11.dll").exists() {
        let _ = fs::copy(dxvk_x64.join("d3d11.dll"), studio_dir.join("d3d11.dll"));
        let _ = fs::copy(dxvk_x64.join("dxgi.dll"), studio_dir.join("dxgi.dll"));
    }
    let dxvk_conf = if data_dir.join("dxvk.conf").exists() {
        data_dir.join("dxvk.conf")
    } else {
        sandbox.join("dxvk.conf")
    };
    if dxvk_conf.exists() {
        let _ = fs::copy(&dxvk_conf, studio_dir.join("dxvk.conf"));
    }

    // 3. Inject FastFlags
    let flags_src = if data_dir.join("ClientAppSettings.json").exists() {
        data_dir.join("ClientAppSettings.json")
    } else {
        sandbox.join("ClientAppSettings.json")
    };
    if flags_src.exists() {
        let settings_dir = studio_dir.join("ClientSettings");
        let _ = fs::create_dir_all(&settings_dir);
        let _ = fs::copy(&flags_src, settings_dir.join("ClientAppSettings.json"));
    }

    // 4. Ensure registry x11 driver
    let reg_file = if data_dir.join("x11_docking_fix.reg").exists() {
        data_dir.join("x11_docking_fix.reg")
    } else {
        sandbox.join("x11_docking_fix.reg")
    };
    if reg_file.exists() {
        let _ = Command::new("wine")
            .arg("regedit")
            .arg(&reg_file)
            .env("WINEPREFIX", &prefix)
            .env("WINEDEBUG", "-all")
            .status();
    }

    // 5. Setup Wine & Universal X11 Docking Hook Environment
    let mut cmd = Command::new("wine");
    cmd.arg(&studio_exe);
    for arg in args {
        cmd.arg(arg);
    }

    cmd.env("WINEPREFIX", &prefix);
    cmd.env("WINEARCH", "win64");

    // Universal Xlib Docking Hook via LD_PRELOAD (works with any Wine version & distro)
    let hook_x64_dir = custom_libs.join("x86_64");
    let hook_x86_dir = custom_libs.join("i386");
    let wine_hook_fallback = if custom_libs.join("wine_hook.so").exists() {
        custom_libs.join("wine_hook.so")
    } else {
        sandbox.join("wine_hook.so")
    };

    if hook_x64_dir.join("wine_hook.so").exists() && hook_x86_dir.join("wine_hook.so").exists() {
        let existing_ld = env::var("LD_LIBRARY_PATH").unwrap_or_default();
        let ld_path = if existing_ld.is_empty() {
            format!("{}:{}", hook_x64_dir.display(), hook_x86_dir.display())
        } else {
            format!("{}:{}:{}", hook_x64_dir.display(), hook_x86_dir.display(), existing_ld)
        };
        cmd.env("LD_LIBRARY_PATH", ld_path);

        let existing_preload = env::var("LD_PRELOAD").unwrap_or_default();
        let preload = if existing_preload.is_empty() {
            "wine_hook.so".to_string()
        } else {
            format!("wine_hook.so:{}", existing_preload)
        };
        cmd.env("LD_PRELOAD", preload);
        cmd.env("WINE_STUDIO_DOCKING_FIX", "1");
        println!("[Cutlery] [Docking Fix] Universal X11 Docking Hook ACTIVE!");
    } else if wine_hook_fallback.exists() {
        let existing_preload = env::var("LD_PRELOAD").unwrap_or_default();
        let preload = if existing_preload.is_empty() {
            wine_hook_fallback.to_string_lossy().to_string()
        } else {
            format!("{}:{}", wine_hook_fallback.display(), existing_preload)
        };
        cmd.env("LD_PRELOAD", preload);
        cmd.env("WINE_STUDIO_DOCKING_FIX", "1");
        println!("[Cutlery] [Docking Fix] Universal X11 Docking Hook ACTIVE!");
    }

    // Enforce X11 / Xwayland (avoids winewayland multi-surface docking crash)
    cmd.env("DISPLAY", env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()));
    cmd.env_remove("WAYLAND_DISPLAY");

    // NTSync / Fsync
    cmd.env("WINEFSYNC", "1");
    cmd.env("WINENTSYNC", "1");
    if Path::new("/dev/ntsync").exists() {
        println!("[Cutlery] /dev/ntsync detected! Kernel NTSync enabled for low-latency synchronization.");
    }

    // DXVK
    cmd.env("DXVK_CONFIG_FILE", &dxvk_conf);
    cmd.env("DXVK_CONFIG", "dxgi.enableDummyCompositionSwapchain=true");
    cmd.env("WINEDLLOVERRIDES", "winegstreamer=d;d3d11=n,b;dxgi=n,b");
    cmd.env("MIMALLOC_FORCE_REDIRECT", "1");

    // Qt Scaling
    cmd.env("QT_AUTO_SCREEN_SCALE_FACTOR", "0");
    cmd.env("QT_SCALE_FACTOR", "1");

    println!("[Cutlery] Launching Roblox Studio: {}", studio_exe.display());
    println!("--------------------------------------------------------");

    cmd.status()
}

fn cmd_mcp(args: &[String]) -> Result<ExitStatus, std::io::Error> {
    let root = get_root_dir();
    let bridge_py = root.join("mcp").join("bridge.py");

    if !bridge_py.exists() {
        eprintln!("[Cutlery] Error: mcp/bridge.py not found in '{}'", root.display());
        std::process::exit(1);
    }

    let mut cmd = Command::new("python3");
    cmd.arg(&bridge_py);
    for arg in args {
        cmd.arg(arg);
    }

    cmd.status()
}

fn cmd_install() -> Result<ExitStatus, std::io::Error> {
    let root = get_root_dir();
    let install_sh = if root.join("scripts").join("install_studio.sh").exists() {
        root.join("scripts").join("install_studio.sh")
    } else {
        root.join("sandbox").join("install_studio.sh")
    };

    if !install_sh.exists() {
        eprintln!("[Cutlery] Error: scripts/install_studio.sh not found.");
        std::process::exit(1);
    }

    Command::new("bash").arg(&install_sh).status()
}

fn cmd_register_desktop() -> Result<(), std::io::Error> {
    let root = get_root_dir();
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let home_path = PathBuf::from(&home);
    let apps_dir = home_path.join(".local").join("share").join("applications");
    let icons_dir = home_path.join(".local").join("share").join("icons");
    let icons_base = icons_dir.join("hicolor");
    let pixmaps_dir = home_path.join(".local").join("share").join("pixmaps");

    fs::create_dir_all(&apps_dir)?;
    fs::create_dir_all(&icons_base)?;
    fs::create_dir_all(&pixmaps_dir)?;

    // Clean up stale previous launcher files that may conflict in KDE/GNOME
    let _ = fs::remove_file(apps_dir.join("cutlery-studio.desktop"));
    let _ = fs::remove_file(apps_dir.join("roblox-studio-fork.desktop"));
    let _ = fs::remove_file(icons_base.join("scalable").join("apps").join("cutlery.svg"));

    // Ensure hicolor has a valid index.theme (required by KDE KIconLoader)
    let hicolor_index = icons_base.join("index.theme");
    if !hicolor_index.exists() {
        if Path::new("/usr/share/icons/hicolor/index.theme").exists() {
            let _ = fs::copy("/usr/share/icons/hicolor/index.theme", &hicolor_index);
        }
    }

    // 1. Copy multi-resolution icons to standard FreeDesktop hicolor directories
    let icon_sizes = [
        (512, root.join("assets").join("cutlery.png")),
        (256, root.join("assets").join("icons").join("cutlery-256x256.png")),
        (128, root.join("assets").join("icons").join("cutlery-128x128.png")),
        (64,  root.join("assets").join("icons").join("cutlery-64x64.png")),
        (48,  root.join("assets").join("icons").join("cutlery-48x48.png")),
        (32,  root.join("assets").join("icons").join("cutlery-32x32.png")),
        (16,  root.join("assets").join("icons").join("cutlery-16x16.png")),
    ];

    for (size, path) in &icon_sizes {
        if path.exists() {
            let target_dir = icons_base.join(format!("{}x{}", size, size)).join("apps");
            let _ = fs::create_dir_all(&target_dir);
            let _ = fs::copy(path, target_dir.join("cutlery.png"));
        }
    }

    // 2. Also inject icons into any custom KDE/user icon themes in ~/.local/share/icons (e.g. BigSur, Whitesur)
    if let Ok(entries) = fs::read_dir(&icons_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.file_name().map_or(false, |n| n != "hicolor") {
                for (size, icon_src) in &icon_sizes {
                    if icon_src.exists() {
                        let theme_target = path.join(format!("{}x{}", size, size)).join("apps");
                        if theme_target.exists() || path.join("index.theme").exists() {
                            let _ = fs::create_dir_all(&theme_target);
                            let _ = fs::copy(icon_src, theme_target.join("cutlery.png"));
                        }
                    }
                }
                let _ = Command::new("gtk-update-icon-cache")
                    .args(["-f", "-t", &path.to_string_lossy()])
                    .status();
            }
        }
    }

    // 3. Root pixmaps and icons fallback
    let main_png = root.join("assets").join("cutlery.png");
    if main_png.exists() {
        let _ = fs::copy(&main_png, pixmaps_dir.join("cutlery.png"));
        let _ = fs::copy(&main_png, icons_dir.join("cutlery.png"));
    }

    // 4. Scalable SVG if present
    let src_svg = root.join("assets").join("cutlery.svg");
    if src_svg.exists() {
        let scalable_dir = icons_base.join("scalable").join("apps");
        let _ = fs::create_dir_all(&scalable_dir);
        let _ = fs::copy(&src_svg, scalable_dir.join("cutlery.svg"));
    }

    // Resolve cutlery binary path
    let cutlery_bin = env::current_exe()
        .unwrap_or_else(|_| root.join("bin").join("cutlery"));
    let escaped_bin = escape_desktop_exec(&cutlery_bin.to_string_lossy());

    let desktop_content = format!(
        "[Desktop Entry]\n\
         Name=Cutlery\n\
         Comment=Roblox Studio Launcher for Linux\n\
         Exec=\"{}\" run %u\n\
         Icon=cutlery\n\
         Terminal=false\n\
         Type=Application\n\
         Categories=Development;IDE;Game;\n\
         MimeType=x-scheme-handler/roblox-studio;\n\
         StartupWMClass=robloxstudiobeta.exe\n",
        escaped_bin
    );

    let desktop_file = apps_dir.join("cutlery.desktop");
    fs::write(&desktop_file, desktop_content)?;

    // Register MIME handler
    let _ = Command::new("xdg-mime")
        .args(["default", "cutlery.desktop", "x-scheme-handler/roblox-studio"])
        .status();

    let _ = Command::new("update-desktop-database")
        .arg(&apps_dir)
        .status();

    let _ = Command::new("gtk-update-icon-cache")
        .args(["-f", "-t", &icons_base.to_string_lossy()])
        .status();

    // Refresh KDE Plasma sycoca cache if running under KDE
    let _ = Command::new("kbuildsycoca6").status();
    let _ = Command::new("kbuildsycoca5").status();

    println!("[Cutlery] Desktop launcher, icons, and MIME protocol registered successfully!");
    println!("          Desktop File: {}", desktop_file.display());
    println!("          Comment:      Roblox Studio Launcher for Linux");
    println!("          Icons Copied: ~/.local/share/icons/ (hicolor & active user themes) and pixmaps");
    println!("          MIME Handler: x-scheme-handler/roblox-studio -> Cutlery");
    println!("          (Browser links to open Studio or authenticate will now launch Cutlery!)");

    Ok(())
}

fn cmd_doctor() {
    let root = get_root_dir();
    let sandbox = root.join("sandbox");
    let prefix = sandbox.join("prefix");
    let custom_libs = root.join("custom-libs");

    println!("========================================================");
    println!("  Cutlery Doctor - Environment Diagnostics");
    println!("  Roblox Studio Launcher for Linux with Native Docking & AI MCP");
    println!("========================================================");
    println!("Project Root:     {}", root.display());
    println!("Wine Prefix:      {}", prefix.display());

    // 1. Wine
    match Command::new("wine").arg("--version").output() {
        Ok(out) => {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            println!("System Wine:      [OK] {}", ver);
        }
        Err(_) => println!("System Wine:      [FAILED] 'wine' binary not found in PATH!"),
    }

    // 2. NTSync
    if Path::new("/dev/ntsync").exists() {
        println!("Kernel NTSync:    [OK] /dev/ntsync available");
    } else {
        println!("Kernel NTSync:    [NOTICE] /dev/ntsync not found (falling back to standard futex/fsync)");
    }

    // 3. Universal X11 Docking Hook
    let wine_hook = if custom_libs.join("x86_64").join("wine_hook.so").exists() {
        custom_libs.join("x86_64").join("wine_hook.so")
    } else if custom_libs.join("wine_hook.so").exists() {
        custom_libs.join("wine_hook.so")
    } else {
        sandbox.join("wine_hook.so")
    };
    if wine_hook.exists() {
        println!("Docking Fix Hook: [OK] Universal X11 hook ready ({})", wine_hook.display());
    } else {
        println!("Docking Fix Hook: [FAILED] wine_hook.so missing!");
    }

    // 4. DXVK Vulkan Renderer
    let dxvk_dll = sandbox.join("dxvk").join("x64").join("d3d11.dll");
    let studio_dxvk = find_studio_exe(&prefix)
        .and_then(|e| e.parent().map(|p| p.join("d3d11.dll")))
        .map_or(false, |p| p.exists());
    if dxvk_dll.exists() || studio_dxvk {
        println!("DXVK Vulkan:      [OK] DXVK v3.1.1 d3d11.dll & dxgi.dll active");
    } else {
        println!("DXVK Vulkan:      [NOTICE] Not cached yet (will auto-download on 'cutlery run')");
    }

    // 5. Studio Executable
    match find_studio_exe(&prefix) {
        Some(exe) => println!("Roblox Studio:    [OK] {}", exe.display()),
        None => println!("Roblox Studio:    [FAILED] Not installed in prefix."),
    }

    // 6. MCP Bridge
    let bridge = root.join("mcp").join("bridge.py");
    if bridge.exists() {
        println!("Universal MCP:    [OK] {}", bridge.display());
    } else {
        println!("Universal MCP:    [FAILED] mcp/bridge.py missing.");
    }

    println!("========================================================");
}

fn print_help() {
    println!("Cutlery - Roblox Studio Launcher for Linux with Native Docking & AI MCP");
    println!("Usage: cutlery [command] [args...]\n");
    println!("Available commands:");
    println!("  run [args...]          Launch Roblox Studio with native docking fixes and DXVK (default)");
    println!("  mcp [args...]          Run the Universal MCP Bridge (for Claude, Cursor, Antigravity, Codex)");
    println!("  install                Install or update Roblox Studio in the Wine prefix");
    println!("  register-desktop       Register desktop launcher and roblox-studio:// browser URI handler");
    println!("  doctor                 Verify system, Wine, driver, and prefix health");
    println!("  help                   Show this help message");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = if args.len() > 1 {
        args[1].as_str()
    } else {
        "run"
    };

    match command {
        "run" | "start" | "studio" => {
            let pass_args = if args.len() > 2 { &args[2..] } else { &[] };
            if let Err(e) = cmd_run(pass_args) {
                eprintln!("[Cutlery] Failed to launch Studio: {}", e);
                std::process::exit(1);
            }
        }
        "mcp" => {
            let pass_args = if args.len() > 2 { &args[2..] } else { &[] };
            if let Err(e) = cmd_mcp(pass_args) {
                eprintln!("[Cutlery] Failed to run MCP bridge: {}", e);
                std::process::exit(1);
            }
        }
        "install" => {
            if let Err(e) = cmd_install() {
                eprintln!("[Cutlery] Failed to run installer: {}", e);
                std::process::exit(1);
            }
        }
        "register-desktop" | "register-mime" => {
            if let Err(e) = cmd_register_desktop() {
                eprintln!("[Cutlery] Failed to register desktop launcher: {}", e);
                std::process::exit(1);
            }
        }
        "doctor" | "status" | "check" => {
            cmd_doctor();
        }
        "help" | "--help" | "-h" => {
            print_help();
        }
        unknown => {
            // If unknown argument starts with roblox-studio: (URI launch from browser) or a file path
            if unknown.starts_with("roblox-studio:") || unknown.ends_with(".rbxl") || unknown.ends_with(".rbxlx") {
                let pass_args = &args[1..];
                if let Err(e) = cmd_run(pass_args) {
                    eprintln!("[Cutlery] Failed to launch Studio with URI: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("[Cutlery] Unknown command: '{}'", unknown);
                print_help();
                std::process::exit(1);
            }
        }
    }
}

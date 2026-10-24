use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub mod config;
pub mod studio_theme;
pub mod theme;
pub mod ui;
pub mod update;

use config::CutleryConfig;

pub(crate) fn get_root_dir() -> PathBuf {
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

pub(crate) fn find_studio_exe(prefix: &Path) -> Option<PathBuf> {
    let drive_c = prefix.join("drive_c");
    if !drive_c.exists() {
        return None;
    }

    // Search common version directories; after a Studio update several can exist,
    // so prefer the most recently installed one.
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
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
                    if let Ok(meta) = fs::metadata(&exe) {
                        let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                        if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
                            newest = Some((modified, exe));
                        }
                    }
                }
            }
        }
    }
    newest.map(|(_, exe)| exe)
}

pub(crate) const DXVK_VERSION: &str = "3.1.1";
const DXVK_SHA256: &str = "40565b4a724aadc4433fa4e010b4b23916d9b1f1baeee64e17186db94f54e608";

pub(crate) fn sanitize_studio_arg(arg: &str) -> Result<String, String> {
    // 1. Check for null bytes, carriage returns, newlines, and forbidden control characters
    if arg.chars().any(|c| c.is_control() || c == '\0') {
        return Err("Argument contains forbidden control characters".to_string());
    }

    // 2. Reject quotes which could alter Windows command line quoting in Wine
    if arg.contains('"') || arg.contains('\'') {
        return Err("Argument contains forbidden quotes".to_string());
    }

    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return Err("Argument is empty".to_string());
    }

    // 3. Protocol URI handler (roblox-studio: or roblox-studio://)
    if trimmed.starts_with("roblox-studio:") {
        // Safe character set for roblox-studio URI:
        // alphanumeric, +, :, -, _, ., =, &, ?, /, %, ,, ~, @
        let is_valid_uri = trimmed.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || c == '+'
                || c == ':'
                || c == '-'
                || c == '_'
                || c == '.'
                || c == '='
                || c == '&'
                || c == '?'
                || c == '/'
                || c == '%'
                || c == ','
                || c == '~'
                || c == '@'
        });

        if !is_valid_uri {
            return Err("roblox-studio URI contains invalid or unsafe characters".to_string());
        }

        return Ok(trimmed.to_string());
    }

    // 4. Place files (.rbxl / .rbxlx)
    if trimmed.ends_with(".rbxl") || trimmed.ends_with(".rbxlx") {
        if trimmed.chars().any(|c| c == '|' || c == '<' || c == '>' || c == '&' || c == ';') {
            return Err("Place file path contains invalid shell characters".to_string());
        }
        return Ok(trimmed.to_string());
    }

    // 5. Standard CLI flags / parameters
    if trimmed.chars().any(|c| c == ';' || c == '|' || c == '&' || c == '`' || c == '$' || c == '<' || c == '>') {
        return Err("Argument contains unsafe shell metacharacters".to_string());
    }

    Ok(trimmed.to_string())
}

pub(crate) fn ensure_docking_hooks(custom_libs: &Path) {
    let hook_x64 = custom_libs.join("x86_64").join("wine_hook.so");
    let hook_x86 = custom_libs.join("i386").join("wine_hook.so");
    let hook_root = custom_libs.join("wine_hook.so");

    if (!hook_x64.exists() || !hook_x86.exists()) && !hook_root.exists() {
        let makefile = custom_libs.join("Makefile");
        let source_c = custom_libs.join("wine_hook.c");
        if makefile.exists() && source_c.exists() {
            println!("[Cutlery] Docking hook binaries not found. Compiling from source (custom-libs/wine_hook.c)...");
            let _ = Command::new("make")
                .args(["-C", &custom_libs.to_string_lossy(), "all"])
                .status();
        }
    }
}

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

    println!("[Cutlery] DXVK not found locally. Downloading DXVK v{} (with dummy composition swapchain support)...", DXVK_VERSION);
    let dxvk_root = sandbox.join("dxvk");
    let _ = fs::create_dir_all(&dxvk_root);
    let tarball = sandbox.join(format!("dxvk-{}.tar.gz", DXVK_VERSION));
    let url = format!(
        "https://github.com/doitsujin/dxvk/releases/download/v{0}/dxvk-{0}.tar.gz",
        DXVK_VERSION
    );

    let curl_status = Command::new("curl")
        .args(["-fSL", &url, "-o", &tarball.to_string_lossy()])
        .status();

    if let Ok(status) = curl_status {
        if status.success() {
            let checksum_check = Command::new("sha256sum")
                .args(["-c", "--status"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    if let Some(mut stdin) = child.stdin.take() {
                        use std::io::Write;
                        let _ = write!(stdin, "{}  {}\n", DXVK_SHA256, tarball.display());
                    }
                    child.wait()
                });

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
                    let _ = fs::write(dxvk_root.join("VERSION"), format!("{}\n", DXVK_VERSION));
                    if sandbox_dxvk.join("d3d11.dll").exists() {
                        println!("[Cutlery] DXVK v{} downloaded, verified (SHA-256), and extracted successfully.", DXVK_VERSION);
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

pub(crate) fn cmd_run(args: &[String]) -> Result<ExitStatus, std::io::Error> {
    let root = get_root_dir();
    let config = CutleryConfig::load();

    let data_dir = root.join("data");
    let sandbox = root.join("sandbox");
    let prefix = config.prefix_dir(&root);
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

    // 2. Sync DXVK dlls and config (auto-downloads DXVK_VERSION if missing on fresh clones)
    let dxvk_x64 = ensure_dxvk(&sandbox, &data_dir);
    if dxvk_x64.join("d3d11.dll").exists() {
        let _ = fs::copy(dxvk_x64.join("d3d11.dll"), studio_dir.join("d3d11.dll"));
        let _ = fs::copy(dxvk_x64.join("dxgi.dll"), studio_dir.join("dxgi.dll"));
    }

    // 3. Sync preferences from config to studio files
    config.sync_studio_files(studio_dir);

    // 4. Ensure registry x11 driver
    let reg_file = if data_dir.join("x11_docking_fix.reg").exists() {
        data_dir.join("x11_docking_fix.reg")
    } else {
        sandbox.join("x11_docking_fix.reg")
    };
    if reg_file.exists() {
        let _ = Command::new(&config.general.wine_binary)
            .arg("regedit")
            .arg(&reg_file)
            .env("WINEPREFIX", &prefix)
            .env("WINEDEBUG", "-all")
            .status();
    }

    // 5. Setup Wine & Universal X11 Docking Hook Environment
    let mut cmd = Command::new(&config.general.wine_binary);
    cmd.arg(&studio_exe);
    for raw_arg in args {
        match sanitize_studio_arg(raw_arg) {
            Ok(clean_arg) => {
                cmd.arg(clean_arg);
            }
            Err(e) => {
                eprintln!("[Cutlery] Security error: {}", e);
                eprintln!("[Cutlery] Rejected argument: '{}'", raw_arg);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Security rejection: {}", e),
                ));
            }
        }
    }

    cmd.env("WINEPREFIX", &prefix);
    cmd.env("WINEARCH", "win64");

    // Universal Xlib Docking Hook via LD_PRELOAD
    if config.kernel.docking_fix {
        ensure_docking_hooks(&custom_libs);
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
    }

    // Enforce X11 / Xwayland
    if config.kernel.force_x11 {
        cmd.env("DISPLAY", env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()));
        cmd.env_remove("WAYLAND_DISPLAY");
    }

    // NTSync / Fsync
    if config.kernel.fsync {
        cmd.env("WINEFSYNC", "1");
    }
    if config.kernel.ntsync {
        cmd.env("WINENTSYNC", "1");
        if Path::new("/dev/ntsync").exists() {
            println!("[Cutlery] /dev/ntsync detected! Kernel NTSync enabled for low-latency synchronization.");
        }
    }

    // DXVK
    let dxvk_conf = studio_dir.join("dxvk.conf");
    cmd.env("DXVK_CONFIG_FILE", &dxvk_conf);
    if config.graphics.dummy_composition {
        cmd.env("DXVK_CONFIG", "dxgi.enableDummyCompositionSwapchain=true");
    }
    cmd.env("WINEDLLOVERRIDES", "winegstreamer=d;d3d11=n,b;dxgi=n,b");
    cmd.env("MIMALLOC_FORCE_REDIRECT", "1");

    // Qt Scaling
    if config.graphics.dpi_scale == "auto" {
        cmd.env("QT_AUTO_SCREEN_SCALE_FACTOR", "1");
    } else {
        cmd.env("QT_AUTO_SCREEN_SCALE_FACTOR", "0");
        cmd.env("QT_SCALE_FACTOR", &config.graphics.dpi_scale);
    }

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

pub(crate) fn cmd_install() -> Result<ExitStatus, std::io::Error> {
    let root = get_root_dir();
    let install_sh = if root.join("scripts").join("install_studio.sh").exists() {
        root.join("scripts").join("install_studio.sh")
    } else {
        root.join("sandbox").join("install_studio.sh")
    };

    if !install_sh.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "scripts/install_studio.sh not found",
        ));
    }

    Command::new("bash").arg(&install_sh).status()
}

/// `cutlery update`: offers a newer Cutlery release, then updates Roblox Studio if needed.
fn cmd_update() -> Result<(), String> {
    let root = get_root_dir();
    let current = update::installed_version(&root);
    println!("[Cutlery] Installed version: {}", current);

    match update::fetch_latest_release() {
        Ok(release) if update::is_newer(&current, &release.version) => {
            if update::is_source_checkout(&root) {
                println!("[Cutlery] {} is available. This is a source checkout; update it with git pull.", release.version);
            } else {
                print!("[Cutlery] Update Cutlery {} -> {}? [y/N] ", current, release.version);
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let mut answer = String::new();
                let _ = std::io::stdin().read_line(&mut answer);
                if answer.trim().eq_ignore_ascii_case("y") {
                    let binary = update::apply_release(&release, &root)?;
                    println!("[Cutlery] Updated to {} ({}).", release.version, binary.display());
                }
            }
        }
        Ok(_) => println!("[Cutlery] Cutlery {} is up to date.", current),
        Err(e) => eprintln!("[Cutlery] Could not check for Cutlery updates: {}", e),
    }

    let prefix = CutleryConfig::load().prefix_dir(&root);
    match update::check_studio(&prefix) {
        update::StudioCheck::UpToDate(version) => println!("[Cutlery] Roblox Studio is up to date ({}).", version),
        update::StudioCheck::Unknown(e) => eprintln!("[Cutlery] Could not check the Roblox Studio version: {}", e),
        update::StudioCheck::NeedsInstall(version) => {
            println!("[Cutlery] Installing Roblox Studio {}...", version.as_deref().unwrap_or("latest"));
            cmd_install().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
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

    // 1. Copy multi-resolution icon assets
    let sizes = ["16x16", "32x32", "48x48", "64x64", "128x128", "256x256"];
    for size in sizes {
        let src_icon = root.join("assets").join("icons").join(format!("cutlery-{}.png", size));
        if src_icon.exists() {
            let dest_hicolor = icons_base.join(size).join("apps");
            let _ = fs::create_dir_all(&dest_hicolor);
            let _ = fs::copy(&src_icon, dest_hicolor.join("cutlery.png"));

            let theme_apps = icons_dir.join(size).join("apps");
            let _ = fs::create_dir_all(&theme_apps);
            let _ = fs::copy(&src_icon, theme_apps.join("cutlery.png"));
        }
    }

    // 2. Refresh icon caches
    if let Ok(entries) = fs::read_dir(&icons_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
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
        .unwrap_or_else(|_| root.join("target").join("release").join("cutlery"));
    let escaped_bin = escape_desktop_exec(&cutlery_bin.to_string_lossy());

    // Exec opens the Settings UI by default from the app drawer/tray
    let desktop_content = format!(
        "[Desktop Entry]\n\
         Name=Cutlery\n\
         Comment=Roblox Studio Launcher & Control Center for Linux\n\
         Exec=\"{}\"\n\
         Icon=cutlery\n\
         Terminal=false\n\
         Type=Application\n\
         Categories=Development;IDE;Game;\n\
         MimeType=x-scheme-handler/roblox-studio;\n\
         StartupWMClass=robloxstudiobeta.exe\n\
         Actions=LaunchStudio;\n\n\
         [Desktop Action LaunchStudio]\n\
         Name=Launch Roblox Studio Directly\n\
         Exec=\"{}\" run %u\n",
        escaped_bin,
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
    println!("          Comment:      Roblox Studio Launcher & Control Center for Linux");
    println!("          App Drawer:   Clicking Cutlery will now open the Control Center UI.");
    println!("          MIME Handler: x-scheme-handler/roblox-studio -> Cutlery");

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
    ensure_docking_hooks(&custom_libs);
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
        println!("DXVK Vulkan:      [OK] DXVK v{} d3d11.dll & dxgi.dll active", DXVK_VERSION);
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
    println!("  ui, settings, config   Open the Settings & Pre-launch Control Center UI (default)");
    println!("  run [args...]          Launch Roblox Studio with native docking fixes and DXVK");
    println!("  mcp [args...]          Run the Universal MCP Bridge (for Claude, Cursor, Antigravity, Codex)");
    println!("  install                Install or update Roblox Studio in the Wine prefix");
    println!("  update                 Update Cutlery (GitHub Releases) and Roblox Studio");
    println!("  register-desktop       Register desktop launcher and roblox-studio:// browser URI handler");
    println!("  doctor                 Verify system, Wine, driver, and prefix health");
    println!("  version                Show version information");
    println!("  help                   Show this help message");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = if args.len() > 1 {
        args[1].as_str()
    } else {
        "ui"
    };

    match command {
        "ui" | "settings" | "config" => {
            if let Err(e) = ui::launch_ui() {
                eprintln!("[Cutlery] Failed to open Control Center UI: {}", e);
                std::process::exit(1);
            }
        }
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
        "update" | "upgrade" => {
            if let Err(e) = cmd_update() {
                eprintln!("[Cutlery] Update failed: {}", e);
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
        "version" | "--version" | "-v" => {
            println!("Cutlery v{}", env!("CARGO_PKG_VERSION"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_valid_roblox_studio_uri() {
        let uri = "roblox-studio:1+launchmode:edit+task:EditPlace+placeId:123456+universeId:789";
        let res = sanitize_studio_arg(uri);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), uri);
    }

    #[test]
    fn test_sanitize_rejects_quotes() {
        let malicious_arg1 = "roblox-studio:1\" -malicious_flag";
        assert!(sanitize_studio_arg(malicious_arg1).is_err());

        let malicious_arg2 = "roblox-studio:1' -flag";
        assert!(sanitize_studio_arg(malicious_arg2).is_err());
    }

    #[test]
    fn test_sanitize_rejects_control_characters_and_newlines() {
        let arg = "roblox-studio:1\n-task:EditPlace";
        assert!(sanitize_studio_arg(arg).is_err());

        let null_byte_arg = "roblox-studio:1\0bad";
        assert!(sanitize_studio_arg(null_byte_arg).is_err());
    }

    #[test]
    fn test_sanitize_rejects_shell_metacharacters() {
        assert!(sanitize_studio_arg("roblox-studio:1; rm -rf /").is_err());
        assert!(sanitize_studio_arg("roblox-studio:1 | ls").is_err());
        assert!(sanitize_studio_arg("roblox-studio:1 & whoami").is_err());
        assert!(sanitize_studio_arg("roblox-studio:1 `whoami`").is_err());
        assert!(sanitize_studio_arg("roblox-studio:1 $(whoami)").is_err());
    }

    #[test]
    fn test_sanitize_valid_place_file() {
        let place = "/home/user/games/my_map.rbxl";
        assert!(sanitize_studio_arg(place).is_ok());

        let place_xml = "my_map.rbxlx";
        assert!(sanitize_studio_arg(place_xml).is_ok());
    }

    #[test]
    fn test_sanitize_standard_cli_flags() {
        assert!(sanitize_studio_arg("-ide").is_ok());
        assert!(sanitize_studio_arg("-task EditPlace").is_ok());
    }
}

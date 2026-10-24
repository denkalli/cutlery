//! Cutlery Control Center: wires the Slint UI (ui/app.slint) to config, environment and actions.

mod env;
mod integrations;
mod state;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use slint::winit_030::{winit::window::ResizeDirection, WinitWindowAccessor};
use slint::ComponentHandle;

use crate::config::CutleryConfig;
use crate::{cmd_install, find_studio_exe, get_root_dir};
use env::EnvReport;

slint::include_modules!();

/// What `EnvReport::probe` needs; `Send` so worker threads can own a copy.
#[derive(Clone)]
struct ProbeTarget {
    root: PathBuf,
    prefix: PathBuf,
    wine_binary: String,
}

/// Shared by the UI callbacks: paths plus the last saved config (the dirty-tracking baseline).
struct Ctx {
    probe: ProbeTarget,
    prefix: PathBuf,
    saved: RefCell<CutleryConfig>,
}

pub fn launch_ui() -> Result<(), Box<dyn std::error::Error>> {
    let app = AppWindow::new()?;
    let root = get_root_dir();
    let config = CutleryConfig::load();
    let prefix = config.prefix_dir(&root);

    let env = app.global::<Env>();
    env.set_client_version(env::client_version().into());
    env.set_prefix_path(prefix.to_string_lossy().to_string().into());
    env.set_dxvk_version(crate::DXVK_VERSION.into());
    env.set_changelog(env::current_changelog().into());
    let (linked, link_path) = integrations::check_antigravity_link();
    env.set_antigravity_linked(linked);
    env.set_antigravity_path(link_path.into());

    state::push_config(&app, &config);
    let ctx = Rc::new(Ctx {
        probe: ProbeTarget {
            root,
            prefix: prefix.clone(),
            wine_binary: config.general.wine_binary.clone(),
        },
        prefix,
        saved: RefCell::new(config),
    });

    register_settings_callbacks(&app, &ctx);
    register_action_callbacks(&app, &ctx);
    register_window_callbacks(&app);

    set_status(&app, "Checking environment…");
    refresh_environment(&app, ctx.probe.clone(), None);

    app.run()?;
    Ok(())
}

fn set_status(app: &AppWindow, text: impl Into<slint::SharedString>) {
    app.global::<Status>().set_text(text.into());
}

fn begin_job(app: &AppWindow) {
    let status = app.global::<Status>();
    status.set_jobs(status.get_jobs() + 1);
}

fn end_job(app: &AppWindow) {
    let status = app.global::<Status>();
    status.set_jobs((status.get_jobs() - 1).max(0));
}

/// Probes the environment on a worker thread and publishes the result to the UI.
/// The status bar then shows `message`, or the probe summary when there is none.
fn refresh_environment(app: &AppWindow, target: ProbeTarget, message: Option<String>) {
    begin_job(app);
    let weak = app.as_weak();
    std::thread::spawn(move || {
        let report = EnvReport::probe(&target.root, &target.prefix, &target.wine_binary);
        let _ = weak.upgrade_in_event_loop(move |app| {
            let env = app.global::<Env>();
            env.set_studio_version(report.studio_version().into());
            env.set_studio_installed(report.studio_exe.is_some());
            env.set_wine_version(report.wine_version.clone().unwrap_or_else(|| "Not found".into()).into());
            env.set_ntsync_available(report.ntsync);
            env.set_checks(slint::ModelRc::new(slint::VecModel::from(report.checks())));
            set_status(&app, message.unwrap_or_else(|| report.summary()));
            end_job(&app);
        });
    });
}

fn apply_settings(app: &AppWindow, ctx: &Ctx) {
    let cfg = state::pull_config(app, &ctx.saved.borrow());
    match cfg.save() {
        Ok(()) => {
            if let Some(studio_dir) = find_studio_exe(&ctx.prefix).and_then(|exe| exe.parent().map(Path::to_path_buf)) {
                cfg.sync_studio_files(&studio_dir);
            }
            *ctx.saved.borrow_mut() = cfg;
            set_status(app, "Settings applied.");
        }
        Err(e) => set_status(app, format!("Failed to save settings: {}", e)),
    }
    state::refresh_dirty(app, &ctx.saved.borrow());
}

fn revert_settings(app: &AppWindow, ctx: &Ctx) {
    state::push_config(app, &ctx.saved.borrow());
    state::refresh_dirty(app, &ctx.saved.borrow());
    set_status(app, "Changes reverted.");
}

fn quit(app: &AppWindow) {
    let _ = app.hide();
    let _ = slint::quit_event_loop();
}

fn launch_studio_and_quit(app: &AppWindow) {
    set_status(app, "Launching Roblox Studio…");
    match std::env::current_exe().and_then(|exe| Command::new(exe).arg("run").spawn()) {
        Ok(_) => quit(app),
        Err(e) => set_status(app, format!("Failed to launch Roblox Studio: {}", e)),
    }
}

/// Runs `action` now, or asks first when there are unapplied changes.
fn guard_unapplied(app: &AppWindow, action: PendingAction) -> bool {
    if app.global::<Settings>().get_dirty() {
        let shell = app.global::<Shell>();
        shell.set_pending_action(action);
        shell.set_confirm_open(true);
        return false;
    }
    true
}

fn register_settings_callbacks(app: &AppWindow, ctx: &Rc<Ctx>) {
    let settings = app.global::<Settings>();

    settings.on_changed({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                state::refresh_dirty(&app, &ctx.saved.borrow());
            }
        }
    });

    settings.on_add_custom_flag({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move |name, value| {
            let (name, value) = (name.trim(), value.trim());
            if name.is_empty() || value.is_empty() {
                return;
            }
            if let Some(app) = weak.upgrade() {
                let s = app.global::<Settings>();
                let mut flags = state::custom_flags(&s);
                flags.insert(name.to_string(), value.to_string());
                state::set_custom_flags(&s, &flags);
                state::refresh_dirty(&app, &ctx.saved.borrow());
            }
        }
    });

    settings.on_remove_custom_flag({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move |name| {
            if let Some(app) = weak.upgrade() {
                let s = app.global::<Settings>();
                let mut flags = state::custom_flags(&s);
                if flags.remove(name.as_str()).is_some() {
                    state::set_custom_flags(&s, &flags);
                    state::refresh_dirty(&app, &ctx.saved.borrow());
                }
            }
        }
    });

    settings.on_apply({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                apply_settings(&app, &ctx);
            }
        }
    });

    settings.on_revert({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                revert_settings(&app, &ctx);
            }
        }
    });
}

fn register_action_callbacks(app: &AppWindow, ctx: &Rc<Ctx>) {
    let actions = app.global::<Actions>();

    actions.on_launch_studio({
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                if guard_unapplied(&app, PendingAction::Launch) {
                    launch_studio_and_quit(&app);
                }
            }
        }
    });

    actions.on_confirm_proceed({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move |action, save| {
            if let Some(app) = weak.upgrade() {
                if save {
                    apply_settings(&app, &ctx);
                }
                match action {
                    PendingAction::Close => quit(&app),
                    PendingAction::Launch => launch_studio_and_quit(&app),
                }
            }
        }
    });

    actions.on_check_updates({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            let Some(app) = weak.upgrade() else { return };
            if app.global::<Status>().get_busy() {
                return;
            }
            begin_job(&app);
            set_status(&app, "Running the Roblox Studio installer/updater…");
            let weak = app.as_weak();
            let target = ctx.probe.clone();
            std::thread::spawn(move || {
                let message = match cmd_install() {
                    Ok(status) if status.success() => "Roblox Studio install/update finished.".to_string(),
                    Ok(status) => format!("Installer exited with {}.", status),
                    Err(e) => format!("Installer failed: {}", e),
                };
                // Re-detect the Studio install, keeping the installer message visible.
                let _ = weak.upgrade_in_event_loop(move |app| {
                    end_job(&app);
                    refresh_environment(&app, target, Some(message));
                });
            });
        }
    });

    actions.on_run_doctor({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                if !app.global::<Status>().get_busy() {
                    set_status(&app, "Running diagnostics…");
                    refresh_environment(&app, ctx.probe.clone(), Some("Diagnostics refreshed.".to_string()));
                }
            }
        }
    });

    actions.on_open_prefix({
        let prefix = ctx.prefix.clone();
        move || {
            let _ = Command::new("xdg-open").arg(&prefix).spawn();
        }
    });

    actions.on_link_antigravity({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            match integrations::link_antigravity() {
                Ok(path) => {
                    let env = app.global::<Env>();
                    env.set_antigravity_linked(true);
                    env.set_antigravity_path(path.to_string_lossy().to_string().into());
                    set_status(&app, "Antigravity MCP plugin linked to Cutlery.");
                }
                Err(e) => set_status(&app, e),
            }
        }
    });

    actions.on_open_antigravity_folder(move || {
        if let Some(dir) = integrations::antigravity_plugin_dir() {
            let _ = Command::new("xdg-open").arg(dir).spawn();
        }
    });

    actions.on_configure_claude({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            match integrations::configure_claude_desktop() {
                Ok(path) => set_status(&app, format!("Claude Desktop configured ({}).", path.display())),
                Err(e) => set_status(&app, e),
            }
        }
    });

    actions.on_test_mcp({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                let (bridge, message) = integrations::probe_mcp_bridge(&ctx.prefix);
                app.global::<Env>().set_mcp_test_status(bridge.into());
                set_status(&app, message);
            }
        }
    });
}

fn register_window_callbacks(app: &AppWindow) {
    let actions = app.global::<Actions>();

    actions.on_minimize_window({
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                app.window().set_minimized(true);
            }
        }
    });

    actions.on_toggle_maximize({
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                let maximized = !app.window().is_maximized();
                app.window().set_maximized(maximized);
                app.global::<Shell>().set_maximized(maximized);
            }
        }
    });

    actions.on_close_window({
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                if guard_unapplied(&app, PendingAction::Close) {
                    quit(&app);
                }
            }
        }
    });

    actions.on_start_resize({
        let weak = app.as_weak();
        move |edge| {
            if let Some(app) = weak.upgrade() {
                let direction = match edge {
                    ResizeEdge::North => ResizeDirection::North,
                    ResizeEdge::South => ResizeDirection::South,
                    ResizeEdge::East => ResizeDirection::East,
                    ResizeEdge::West => ResizeDirection::West,
                    ResizeEdge::NorthEast => ResizeDirection::NorthEast,
                    ResizeEdge::NorthWest => ResizeDirection::NorthWest,
                    ResizeEdge::SouthEast => ResizeDirection::SouthEast,
                    ResizeEdge::SouthWest => ResizeDirection::SouthWest,
                };
                app.window().with_winit_window(|window| {
                    let _ = window.drag_resize_window(direction);
                });
            }
        }
    });

    // Window manager close (Alt+F4, taskbar): same unapplied-changes guard as the close button.
    app.window().on_close_requested({
        let weak = app.as_weak();
        move || match weak.upgrade() {
            Some(app) if !guard_unapplied(&app, PendingAction::Close) => slint::CloseRequestResponse::KeepWindowShown,
            _ => slint::CloseRequestResponse::HideWindow,
        }
    });
}

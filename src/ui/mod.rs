//! Cutlery Control Center: wires the Slint UI (ui/app.slint) to config, environment and actions.

mod env;
mod integrations;
mod state;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use slint::ComponentHandle;

use crate::config::CutleryConfig;
use crate::theme::{self, ThemeDef};
use crate::update::{self, Release, StudioCheck};
use crate::{cmd_install, find_studio_exe, get_root_dir};
use env::EnvReport;

slint::include_modules!();

/// What `EnvReport::probe` needs; `Send` so worker threads can own a copy.
#[derive(Clone)]
struct ProbeTarget {
    root: PathBuf,
    prefix: PathBuf,
    wine_binary: String,
    /// Whether the saved config themes Studio (the Doctor then checks the Studio build).
    studio_theme: bool,
}

/// Shared by the UI callbacks: paths plus the last saved config (the dirty-tracking baseline).
struct Ctx {
    probe: ProbeTarget,
    prefix: PathBuf,
    saved: RefCell<CutleryConfig>,
    /// Cutlery release offered in the UpdateDialog; filled by a worker thread.
    offered_release: Arc<Mutex<Option<Release>>>,
    /// Built-in and user themes, reloaded by the "Reload Themes" button.
    themes: RefCell<Vec<ThemeDef>>,
    /// Problems from the last theme load, shown under the theme picker.
    theme_errors: RefCell<Vec<String>>,
    /// Id of the theme currently painted, so other settings edits skip re-applying it.
    applied_theme: RefCell<String>,
}

pub fn launch_ui() -> Result<(), Box<dyn std::error::Error>> {
    let app = AppWindow::new()?;
    let root = get_root_dir();
    let config = CutleryConfig::load();
    let prefix = config.prefix_dir(&root);

    let installed = update::installed_version(&root);
    let changelog: Vec<ChangelogSection> = env::load_changelog(&root)
        .into_iter()
        .map(|entry| ChangelogSection {
            current: entry.version == installed,
            version: entry.version.into(),
            items: slint::ModelRc::new(slint::VecModel::from(
                entry.items.into_iter().map(slint::SharedString::from).collect::<Vec<_>>(),
            )),
        })
        .collect();

    let env = app.global::<Env>();
    env.set_client_version(installed.into());
    env.set_prefix_path(prefix.to_string_lossy().to_string().into());
    env.set_changelog(slint::ModelRc::new(slint::VecModel::from(changelog)));
    let (linked, link_path) = integrations::check_antigravity_link();
    env.set_antigravity_linked(linked);
    env.set_antigravity_path(link_path.into());
    env.set_themes_dir(theme::user_dir().to_string_lossy().to_string().into());

    state::push_config(&app, &config);
    let ctx = Rc::new(Ctx {
        probe: ProbeTarget {
            root,
            prefix: prefix.clone(),
            wine_binary: config.general.wine_binary.clone(),
            studio_theme: config.appearance.studio_theme,
        },
        prefix,
        saved: RefCell::new(config),
        offered_release: Arc::new(Mutex::new(None)),
        themes: RefCell::new(Vec::new()),
        theme_errors: RefCell::new(Vec::new()),
        applied_theme: RefCell::new(String::new()),
    });
    reload_themes(&app, &ctx);

    register_settings_callbacks(&app, &ctx);
    register_action_callbacks(&app, &ctx);
    register_window_callbacks(&app);

    set_status(&app, "Checking environment…");
    refresh_environment(&app, ctx.probe_target(), None);

    app.run()?;
    Ok(())
}

impl Ctx {
    /// Probe target following the last saved config.
    fn probe_target(&self) -> ProbeTarget {
        ProbeTarget { studio_theme: self.saved.borrow().appearance.studio_theme, ..self.probe.clone() }
    }
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
        let report = EnvReport::probe(&target.root, &target.prefix, &target.wine_binary, target.studio_theme);
        let _ = weak.upgrade_in_event_loop(move |app| {
            let env = app.global::<Env>();
            env.set_studio_version(report.studio_version().into());
            env.set_studio_installed(report.studio_exe.is_some());
            env.set_dxvk_version(report.dxvk_label().into());
            env.set_wine_version(report.wine_version.clone().unwrap_or_else(|| "Not found".into()).into());
            env.set_ntsync_available(report.ntsync);
            env.set_discord_available(report.discord_socket.is_some());
            env.set_checks(slint::ModelRc::new(slint::VecModel::from(report.checks())));
            set_status(&app, message.unwrap_or_else(|| report.summary()));
            end_job(&app);
        });
    });
}

/// Re-reads the built-in and user themes, then re-applies the selected one.
fn reload_themes(app: &AppWindow, ctx: &Ctx) {
    let (themes, errors) = theme::load_all(&theme::user_dir());
    let options: Vec<ThemeOption> = themes.iter().map(state::theme_option).collect();
    app.global::<Env>().set_themes(slint::ModelRc::new(slint::VecModel::from(options)));
    *ctx.themes.borrow_mut() = themes;
    *ctx.theme_errors.borrow_mut() = errors;
    apply_selected_theme(app, ctx);
}

/// Paints the UI with the theme picked in Settings (a live preview until Apply).
/// An unknown id falls back to the default theme without changing the setting.
fn apply_selected_theme(app: &AppWindow, ctx: &Ctx) {
    let selected = app.global::<Settings>().get_theme_name();
    let themes = ctx.themes.borrow();
    let mut errors = ctx.theme_errors.borrow().clone();
    let theme = match themes.iter().find(|t| t.id == selected.as_str()) {
        Some(theme) => Some(theme),
        None => {
            errors.push(format!("theme \"{}\" not found, showing {}", selected, theme::DEFAULT_THEME));
            themes.iter().find(|t| t.id == theme::DEFAULT_THEME)
        }
    };
    if let Some(theme) = theme {
        app.global::<Theme>().set_palette(state::theme_palette(theme));
    }
    app.global::<Env>().set_theme_errors(errors.join("\n").into());
    *ctx.applied_theme.borrow_mut() = selected.to_string();
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
    apply_selected_theme(app, ctx);
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

/// Update button, step 1: look for a newer Cutlery release and offer it in the UpdateDialog.
/// Without one (or when the check fails) it moves straight on to the Studio check.
fn check_cutlery_update(app: &AppWindow, ctx: &Ctx) {
    if app.global::<Status>().get_busy() {
        return;
    }
    begin_job(app);
    set_status(app, "Checking for Cutlery updates…");
    let weak = app.as_weak();
    let root = ctx.probe.root.clone();
    let offered = ctx.offered_release.clone();
    std::thread::spawn(move || {
        let current = update::installed_version(&root);
        let latest = update::fetch_latest_release();
        let _ = weak.upgrade_in_event_loop(move |app| {
            end_job(&app);
            match latest {
                Ok(release) if update::is_newer(&current, &release.version) => {
                    if update::is_source_checkout(&root) {
                        set_status(
                            &app,
                            format!("Cutlery {} is available; this is a source checkout, update it with git pull.", release.version),
                        );
                        app.global::<Actions>().invoke_check_studio_updates();
                        return;
                    }
                    let shell = app.global::<Shell>();
                    shell.set_update_from(current.into());
                    shell.set_update_to(release.version.as_str().into());
                    *offered.lock().unwrap() = Some(release);
                    shell.set_update_open(true);
                    set_status(&app, "A Cutlery update is available.");
                }
                Ok(_) => {
                    set_status(&app, format!("Cutlery {} is up to date.", current));
                    app.global::<Actions>().invoke_check_studio_updates();
                }
                Err(e) => {
                    set_status(&app, format!("Could not check for Cutlery updates: {}", e));
                    app.global::<Actions>().invoke_check_studio_updates();
                }
            }
        });
    });
}

/// Downloads and installs the offered release, then restarts into the new binary.
fn install_offered_release(app: &AppWindow, ctx: &Ctx) {
    let Some(release) = ctx.offered_release.lock().unwrap().take() else { return };
    begin_job(app);
    set_status(app, format!("Downloading and installing Cutlery {}…", release.version));
    let weak = app.as_weak();
    let root = ctx.probe.root.clone();
    std::thread::spawn(move || {
        let result = update::apply_release(&release, &root);
        let _ = weak.upgrade_in_event_loop(move |app| {
            end_job(&app);
            match result.map_err(|e| e.to_string()).and_then(|binary| {
                Command::new(&binary).arg("ui").spawn().map_err(|e| format!("Updated, but could not restart: {}", e))
            }) {
                Ok(_) => quit(&app),
                Err(e) => set_status(&app, e),
            }
        });
    });
}

/// Update button, step 2 (also Doctor → Check Studio Updates): run the Studio installer only
/// when Roblox published a build newer than the installed one.
fn check_studio_update(app: &AppWindow, ctx: &Ctx) {
    if app.global::<Status>().get_busy() {
        return;
    }
    begin_job(app);
    set_status(app, "Checking the latest Roblox Studio version…");
    let weak = app.as_weak();
    let target = ctx.probe_target();
    std::thread::spawn(move || {
        let message = match update::check_studio(&target.prefix) {
            StudioCheck::UpToDate(version) => format!("Roblox Studio is up to date ({}).", version),
            StudioCheck::Unknown(e) => format!("Could not check the Roblox Studio version: {}", e),
            StudioCheck::NeedsInstall(version) => {
                let label = version.unwrap_or_else(|| "latest".into());
                let progress = format!("Installing Roblox Studio {}…", label);
                let _ = weak.upgrade_in_event_loop(move |app| set_status(&app, progress));
                match cmd_install() {
                    Ok(status) if status.success() => format!("Roblox Studio {} installed.", label),
                    Ok(status) => format!("Roblox Studio installer exited with {}.", status),
                    Err(e) => format!("Roblox Studio installer failed: {}", e),
                }
            }
        };

        let _ = weak.upgrade_in_event_loop(move |app| {
            end_job(&app);
            refresh_environment(&app, target, Some(message));
        });
    });
}

fn register_settings_callbacks(app: &AppWindow, ctx: &Rc<Ctx>) {
    let settings = app.global::<Settings>();

    settings.on_changed({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                if app.global::<Settings>().get_theme_name() != ctx.applied_theme.borrow().as_str() {
                    apply_selected_theme(&app, &ctx);
                }
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
                    PendingAction::Update => install_offered_release(&app, &ctx),
                }
            }
        }
    });

    actions.on_check_updates({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                check_cutlery_update(&app, &ctx);
            }
        }
    });

    actions.on_check_studio_updates({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                check_studio_update(&app, &ctx);
            }
        }
    });

    actions.on_accept_update({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                if guard_unapplied(&app, PendingAction::Update) {
                    install_offered_release(&app, &ctx);
                }
            }
        }
    });

    actions.on_decline_update({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                ctx.offered_release.lock().unwrap().take();
                check_studio_update(&app, &ctx);
            }
        }
    });

    actions.on_run_doctor({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                if !app.global::<Status>().get_busy() {
                    set_status(&app, "Running diagnostics…");
                    refresh_environment(&app, ctx.probe_target(), Some("Diagnostics refreshed.".to_string()));
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

    actions.on_open_themes_folder({
        let weak = app.as_weak();
        move || {
            let dir = theme::user_dir();
            match std::fs::create_dir_all(&dir) {
                Ok(()) => {
                    let _ = Command::new("xdg-open").arg(&dir).spawn();
                }
                Err(e) => {
                    if let Some(app) = weak.upgrade() {
                        set_status(&app, format!("Could not create {}: {}", dir.display(), e));
                    }
                }
            }
        }
    });

    actions.on_reload_themes({
        let weak = app.as_weak();
        let ctx = ctx.clone();
        move || {
            if let Some(app) = weak.upgrade() {
                reload_themes(&app, &ctx);
                let count = ctx.themes.borrow().len();
                set_status(&app, format!("{} themes loaded.", count));
            }
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

    // Window manager close (Alt+F4, taskbar): same unapplied-changes guard as the close button.
    app.window().on_close_requested({
        let weak = app.as_weak();
        move || match weak.upgrade() {
            Some(app) if !guard_unapplied(&app, PendingAction::Close) => slint::CloseRequestResponse::KeepWindowShown,
            _ => slint::CloseRequestResponse::HideWindow,
        }
    });
}

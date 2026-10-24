//! Moves settings between `CutleryConfig` and the Slint `Settings` global, and themes into `Theme`.

use std::collections::BTreeMap;

use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::{AppWindow, FastFlagItem, Settings, ThemeOption, ThemePalette};
use crate::config::CutleryConfig;
use crate::theme::{Rgba, ThemeDef};

pub(super) fn push_config(app: &AppWindow, cfg: &CutleryConfig) {
    let s = app.global::<Settings>();
    s.set_theme_name(cfg.appearance.theme.as_str().into());
    s.set_accent_name(cfg.appearance.accent_color.as_str().into());
    s.set_studio_theme(cfg.appearance.studio_theme);
    s.set_always_open_ui(cfg.general.always_open_ui);

    s.set_target_fps(cfg.graphics.target_fps);
    s.set_dummy_composition(cfg.graphics.dummy_composition);
    s.set_dxvk_async(cfg.graphics.dxvk_async);

    s.set_ntsync_enabled(cfg.kernel.ntsync);
    s.set_fsync_enabled(cfg.kernel.fsync);
    s.set_docking_fix(cfg.kernel.docking_fix);
    s.set_force_x11(cfg.kernel.force_x11);

    s.set_fastflags_preset(cfg.fastflags.preset.as_str().into());
    s.set_telemetry_bypass(cfg.fastflags.telemetry_bypass);
    s.set_gpu_light_culling(cfg.fastflags.gpu_light_culling);
    set_custom_flags(&s, &cfg.fastflags.custom);

    s.set_mcp_enabled(cfg.mcp.enabled);
    s.set_preinit_shield(cfg.mcp.preinit_shield);

    s.set_discord_enabled(cfg.discord.enabled);
    s.set_discord_privacy(cfg.discord.privacy_mode);
    s.set_discord_elapsed(cfg.discord.show_elapsed);
    s.set_discord_buttons(cfg.discord.show_buttons);
    s.set_discord_app_id(cfg.discord.application_id.as_str().into());
}

/// Returns `base` with every field the UI edits replaced by its current UI value.
pub(super) fn pull_config(app: &AppWindow, base: &CutleryConfig) -> CutleryConfig {
    let s = app.global::<Settings>();
    let mut cfg = base.clone();
    cfg.appearance.theme = s.get_theme_name().to_string();
    cfg.appearance.accent_color = s.get_accent_name().to_string();
    cfg.appearance.studio_theme = s.get_studio_theme();
    cfg.general.always_open_ui = s.get_always_open_ui();

    cfg.graphics.target_fps = s.get_target_fps();
    cfg.graphics.dummy_composition = s.get_dummy_composition();
    cfg.graphics.dxvk_async = s.get_dxvk_async();

    cfg.kernel.ntsync = s.get_ntsync_enabled();
    cfg.kernel.fsync = s.get_fsync_enabled();
    cfg.kernel.docking_fix = s.get_docking_fix();
    cfg.kernel.force_x11 = s.get_force_x11();

    cfg.fastflags.preset = s.get_fastflags_preset().to_string();
    cfg.fastflags.telemetry_bypass = s.get_telemetry_bypass();
    cfg.fastflags.gpu_light_culling = s.get_gpu_light_culling();
    cfg.fastflags.custom = custom_flags(&s);

    cfg.mcp.enabled = s.get_mcp_enabled();
    cfg.mcp.preinit_shield = s.get_preinit_shield();

    cfg.discord.enabled = s.get_discord_enabled();
    cfg.discord.privacy_mode = s.get_discord_privacy();
    cfg.discord.show_elapsed = s.get_discord_elapsed();
    cfg.discord.show_buttons = s.get_discord_buttons();
    cfg.discord.application_id = s.get_discord_app_id().trim().to_string();
    cfg
}

pub(super) fn refresh_dirty(app: &AppWindow, saved: &CutleryConfig) {
    let dirty = pull_config(app, saved) != *saved;
    app.global::<Settings>().set_dirty(dirty);
}

pub(super) fn custom_flags(s: &Settings<'_>) -> BTreeMap<String, String> {
    s.get_custom_flags()
        .iter()
        .map(|item| (item.name.to_string(), item.value.to_string()))
        .collect()
}

pub(super) fn set_custom_flags(s: &Settings<'_>, flags: &BTreeMap<String, String>) {
    let items: Vec<FastFlagItem> = flags
        .iter()
        .map(|(name, value)| FastFlagItem {
            name: name.as_str().into(),
            value: value.as_str().into(),
        })
        .collect();
    s.set_custom_flags(ModelRc::new(VecModel::from(items)));
}

fn color(c: Rgba) -> slint::Color {
    slint::Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

pub(super) fn theme_option(t: &ThemeDef) -> ThemeOption {
    ThemeOption {
        id: t.id.as_str().into(),
        name: t.name.as_str().into(),
        author: t.author.as_str().into(),
        bg: color(t.color("window-bg")),
        surface: color(t.color("surface")),
        text: color(t.color("text")),
        has_accent: t.accent.is_some(),
        accent: t.accent.map(color).unwrap_or_default(),
    }
}

pub(super) fn theme_palette(t: &ThemeDef) -> ThemePalette {
    let c = |key: &str| color(t.color(key));
    let home_bg = t.background.as_deref().and_then(|path| slint::Image::load_from_path(path).ok());
    ThemePalette {
        dark: t.dark,
        window_bg: c("window-bg"),
        window_border: c("window-border"),
        surface: c("surface"),
        surface_hover: c("surface-hover"),
        surface_sunken: c("surface-sunken"),
        overlay_bg: c("overlay-bg"),
        border: c("border"),
        border_strong: c("border-strong"),
        scrim: c("scrim"),
        shadow: c("shadow"),
        hover_tint: c("hover-tint"),
        status_glass: c("status-glass"),
        card_glass: c("card-glass"),
        sidebar_glass: c("sidebar-glass"),
        hero_top: c("hero-top"),
        hero_bottom: c("hero-bottom"),
        text: c("text"),
        text_soft: c("text-soft"),
        text_muted: c("text-muted"),
        text_faint: c("text-faint"),
        ok: c("ok"),
        info: c("info"),
        warn: c("warn"),
        danger: c("danger"),
        has_accent: t.accent.is_some(),
        accent: t.accent.map(color).unwrap_or_default(),
        has_home_bg: home_bg.is_some(),
        home_bg: home_bg.unwrap_or_default(),
    }
}

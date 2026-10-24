//! Moves settings between `CutleryConfig` and the Slint `Settings` global.

use std::collections::BTreeMap;

use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::{AppWindow, FastFlagItem, Settings};
use crate::config::CutleryConfig;

pub(super) fn push_config(app: &AppWindow, cfg: &CutleryConfig) {
    let s = app.global::<Settings>();
    s.set_accent_name(cfg.graphics.accent_color.as_str().into());
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
}

/// Returns `base` with every field the UI edits replaced by its current UI value.
pub(super) fn pull_config(app: &AppWindow, base: &CutleryConfig) -> CutleryConfig {
    let s = app.global::<Settings>();
    let mut cfg = base.clone();
    cfg.graphics.accent_color = s.get_accent_name().to_string();
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

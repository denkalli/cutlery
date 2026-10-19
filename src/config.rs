use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub prefix_path: String,
    pub wine_binary: String,
    pub always_open_ui: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            prefix_path: String::new(),
            wine_binary: "wine".to_string(),
            always_open_ui: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicsConfig {
    pub target_fps: i32,
    pub dummy_composition: bool,
    pub dxvk_async: bool,
    pub dpi_scale: String,
}

impl Default for GraphicsConfig {
    fn default() -> Self {
        Self {
            target_fps: 60,
            dummy_composition: true,
            dxvk_async: true,
            dpi_scale: "1.0".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelConfig {
    pub ntsync: bool,
    pub fsync: bool,
    pub docking_fix: bool,
    pub force_x11: bool,
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            ntsync: true,
            fsync: true,
            docking_fix: true,
            force_x11: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastFlagsConfig {
    pub preset: String,
    pub telemetry_bypass: bool,
    pub gpu_light_culling: bool,
}

impl Default for FastFlagsConfig {
    fn default() -> Self {
        Self {
            preset: "default".to_string(),
            telemetry_bypass: true,
            gpu_light_culling: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub enabled: bool,
    pub preinit_shield: bool,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            preinit_shield: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CutleryConfig {
    pub general: GeneralConfig,
    pub graphics: GraphicsConfig,
    pub kernel: KernelConfig,
    pub fastflags: FastFlagsConfig,
    pub mcp: McpConfig,
}

impl CutleryConfig {
    pub fn config_path() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(".config")
                .join("cutlery")
                .join("config.toml")
        } else {
            PathBuf::from("config.toml")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str::<CutleryConfig>(&contents) {
                    return config;
                }
            }
        }
        let default_config = Self::default();
        let _ = default_config.save();
        default_config
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(&path, serialized)?;
        Ok(())
    }

    /// Syncs relevant options into ClientAppSettings.json and dxvk.conf in target data or prefix dir
    pub fn sync_studio_files(&self, studio_dir: &Path) {
        let settings_dir = studio_dir.join("ClientSettings");
        let _ = fs::create_dir_all(&settings_dir);
        let settings_file = settings_dir.join("ClientAppSettings.json");

        let mut flags = serde_json::json!({
            "FFlagDebugGraphicsPreferD3D11": "True",
            "FFlagDebugGraphicsDisableDirect3D11": "False",
            "FFlagDebugGraphicsPreferVulkan": "False",
            "FFlagDebugGraphicsDisableOpenGL": "True",
            "FFlagRenderFixImpreciseLightGrid": "True",
            "FFlagStudioMultiSelectDragger": "True"
        });

        if let Some(map) = flags.as_object_mut() {
            map.insert(
                "DFIntTaskSchedulerTargetFps".to_string(),
                serde_json::json!(self.graphics.target_fps),
            );
            if self.fastflags.telemetry_bypass {
                map.insert("FFlagDebugDisableTelemetryWait".to_string(), serde_json::json!("True"));
                map.insert("FFlagDebugDisableTelemetryEvent".to_string(), serde_json::json!("True"));
            }
            if self.fastflags.gpu_light_culling {
                map.insert("FFlagFastGPULightCulling3".to_string(), serde_json::json!("True"));
            }
        }

        if let Ok(json_str) = serde_json::to_string_pretty(&flags) {
            let _ = fs::write(settings_file, json_str);
        }

        // DXVK configuration sync
        let dxvk_conf = studio_dir.join("dxvk.conf");
        let async_val = if self.graphics.dxvk_async { "true" } else { "false" };
        let comp_val = if self.graphics.dummy_composition { "true" } else { "false" };
        let conf_content = format!(
            "# DXVK Configuration managed by Cutlery\n\
             dxvk.enableAsync = {}\n\
             dxvk.gpl = true\n\
             dxvk.numCompilerThreads = 0\n\
             dxvk.syncInterval = 0\n\
             dxvk.tearFree = false\n\
             dxvk.maxChunkSize = 128\n\
             dxgi.enableDummyCompositionSwapchain = {}\n",
            async_val, comp_val
        );
        let _ = fs::write(dxvk_conf, conf_content);
    }
}

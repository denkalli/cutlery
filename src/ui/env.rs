//! Environment detection shown on the Doctor page and in the status bar.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::DoctorCheck;
use crate::{find_studio_exe, DXVK_VERSION};

const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

const LEVEL_OK: i32 = 0;
const LEVEL_WARN: i32 = 1;
const LEVEL_FAIL: i32 = 2;

/// Snapshot of the host and prefix state. Built off the UI thread, so it only holds owned data.
pub(super) struct EnvReport {
    pub wine_version: Option<String>,
    pub ntsync: bool,
    pub dxvk: bool,
    pub hook: bool,
    pub studio_exe: Option<PathBuf>,
}

impl EnvReport {
    /// Probes Wine, NTSync, DXVK, the docking hook and the Studio install.
    /// May compile the docking hook, so call it from a worker thread.
    pub fn probe(root: &Path, prefix: &Path, wine_binary: &str) -> Self {
        let wine_version = Command::new(wine_binary)
            .arg("--version")
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());

        let dxvk = prefix
            .join("drive_c")
            .join("windows")
            .join("system32")
            .join("d3d11.dll")
            .exists()
            || root.join("sandbox").join("dxvk").join("x64").join("d3d11.dll").exists();

        let custom_libs = root.join("custom-libs");
        crate::ensure_docking_hooks(&custom_libs);
        let hook = custom_libs.join("wine_hook.so").exists()
            || custom_libs.join("x86_64").join("wine_hook.so").exists();

        Self {
            wine_version,
            ntsync: Path::new("/dev/ntsync").exists(),
            dxvk,
            hook,
            studio_exe: find_studio_exe(prefix),
        }
    }

    /// Name of the installed Studio version directory, if any.
    pub fn studio_version(&self) -> String {
        match &self.studio_exe {
            Some(exe) => exe
                .parent()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "RobloxStudioBeta.exe".to_string()),
            None => "Not installed".to_string(),
        }
    }

    pub fn checks(&self) -> Vec<DoctorCheck> {
        let check = |label: &str, value: String, level: i32| DoctorCheck {
            label: label.into(),
            value: value.into(),
            level,
        };
        vec![
            match &self.wine_version {
                Some(v) => check("System Wine", v.clone(), LEVEL_OK),
                None => check("System Wine", "Not found".into(), LEVEL_FAIL),
            },
            if self.ntsync {
                check("Kernel NTSync", "OK (/dev/ntsync)".into(), LEVEL_OK)
            } else {
                check("Kernel NTSync", "Fsync fallback".into(), LEVEL_WARN)
            },
            if self.hook {
                check("X11 Docking Hook", "OK (wine_hook.so)".into(), LEVEL_OK)
            } else {
                check("X11 Docking Hook", "Missing".into(), LEVEL_FAIL)
            },
            if self.dxvk {
                check("DXVK Vulkan Layer", format!("OK (v{})", DXVK_VERSION), LEVEL_OK)
            } else {
                check("DXVK Vulkan Layer", "Fetched on first launch".into(), LEVEL_WARN)
            },
            if self.studio_exe.is_some() {
                check("Prefix Integrity", "OK (Studio installed)".into(), LEVEL_OK)
            } else {
                check("Prefix Integrity", "Needs install".into(), LEVEL_WARN)
            },
        ]
    }

    pub fn summary(&self) -> String {
        if self.studio_exe.is_none() {
            "Roblox Studio is not installed yet. Use Update (or Doctor → Install) to install it.".to_string()
        } else if self.wine_version.is_none() {
            "Wine was not found in PATH. Install Wine 9.0+ to launch Studio.".to_string()
        } else {
            format!("Ready • Roblox Studio detected • DXVK v{}", DXVK_VERSION)
        }
    }
}

/// Version shown in the title bar: a `version` file next to the binary (release packages)
/// or the crate version.
pub(super) fn client_version() -> String {
    let from_file = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("version")))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|content| content.trim().to_string())
        .filter(|v| !v.is_empty());
    let version = from_file.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    if version.starts_with('v') {
        version
    } else {
        format!("v{}", version)
    }
}

/// Changelog entries for the running crate version, as plain text.
pub(super) fn current_changelog() -> String {
    changelog_section(CHANGELOG, env!("CARGO_PKG_VERSION"))
        .unwrap_or_else(|| "No changelog entry for this version.".to_string())
}

/// Extracts the `## vX.Y.Z` section of a Markdown changelog and turns `- item` bullets into
/// blank-line separated paragraphs.
fn changelog_section(markdown: &str, version: &str) -> Option<String> {
    let heading = format!("## v{}", version.trim_start_matches('v'));
    let mut lines = markdown.lines();
    lines.find(|line| line.trim() == heading || line.trim().starts_with(&format!("{} ", heading)))?;

    let entries: Vec<String> = lines
        .take_while(|line| !line.starts_with("## "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_start_matches("- ").replace("**", ""))
        .collect();

    if entries.is_empty() {
        None
    } else {
        Some(entries.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\n## v1.3.0 (2026-10-24)\n\n- **New UI**: rebuilt.\n- Faster.\n\n## v1.2.1\n\n- Old entry.\n";

    #[test]
    fn test_changelog_section_extracts_current_version() {
        assert_eq!(
            changelog_section(SAMPLE, "1.3.0").as_deref(),
            Some("New UI: rebuilt.\n\nFaster.")
        );
        assert_eq!(changelog_section(SAMPLE, "v1.2.1").as_deref(), Some("Old entry."));
    }

    #[test]
    fn test_changelog_section_missing_version() {
        assert_eq!(changelog_section(SAMPLE, "9.9.9"), None);
    }

    #[test]
    fn test_bundled_changelog_has_current_version() {
        assert!(changelog_section(CHANGELOG, env!("CARGO_PKG_VERSION")).is_some());
    }
}

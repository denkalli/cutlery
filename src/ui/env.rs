//! Environment detection shown on the Doctor page and in the status bar.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::DoctorCheck;
use crate::{discord, studio_theme};
use crate::update::STUDIO_VERSION_MARKER;
use crate::{find_studio_exe, DXVK_VERSION};

/// Fallback when the install has no CHANGELOG.md (e.g. a bare binary).
const BUNDLED_CHANGELOG: &str = include_str!("../../CHANGELOG.md");

const LEVEL_OK: i32 = 0;
const LEVEL_WARN: i32 = 1;
const LEVEL_FAIL: i32 = 2;
const LEVEL_NEUTRAL: i32 = 3;

/// Snapshot of the host and prefix state. Built off the UI thread, so it only holds owned data.
pub(super) struct EnvReport {
    pub wine_version: Option<String>,
    pub ntsync: bool,
    pub dxvk: bool,
    /// From `sandbox/dxvk/VERSION`, written when Cutlery downloads DXVK.
    pub dxvk_version: Option<String>,
    pub hook: bool,
    pub studio_exe: Option<PathBuf>,
    /// Human-readable Studio version written by the installer next to the executable.
    pub studio_version_id: Option<String>,
    /// `None` while the Studio theme is off (or Studio is not installed).
    pub studio_theme: Option<studio_theme::Status>,
    pub discord_socket: Option<PathBuf>,
}

impl EnvReport {
    /// Probes Wine, NTSync, DXVK, the docking hook, the Studio install and, when `studio_theme`
    /// is on, whether this Studio build can be themed. May compile the docking hook and read the
    /// whole Studio exe, so call it from a worker thread.
    pub fn probe(root: &Path, prefix: &Path, wine_binary: &str, studio_theme: bool) -> Self {
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
            || crate::sandbox_dir(root).join("dxvk").join("x64").join("d3d11.dll").exists();

        let custom_libs = root.join("custom-libs");
        crate::ensure_docking_hooks(&custom_libs);
        let hook = custom_libs.join("wine_hook.so").exists()
            || custom_libs.join("x86_64").join("wine_hook.so").exists();

        let studio_exe = find_studio_exe(prefix);
        let studio_version_id = studio_exe
            .as_ref()
            .and_then(|exe| exe.parent())
            .and_then(|dir| read_trimmed(&dir.join(STUDIO_VERSION_MARKER)));
        let studio_theme = studio_exe
            .as_ref()
            .and_then(|exe| exe.parent())
            .filter(|_| studio_theme)
            .map(studio_theme::status);

        Self {
            wine_version,
            ntsync: Path::new("/dev/ntsync").exists(),
            dxvk,
            dxvk_version: read_trimmed(&crate::sandbox_dir(root).join("dxvk").join("VERSION")),
            hook,
            studio_exe,
            studio_version_id,
            studio_theme,
            discord_socket: discord::find_socket(),
        }
    }

    /// DXVK version in use, `v`-prefixed.
    pub fn dxvk_label(&self) -> String {
        let version = self.dxvk_version.as_deref().unwrap_or(DXVK_VERSION);
        format!("v{}", version.trim_start_matches('v'))
    }

    /// Installed Studio version: the installer's version marker, else the version directory name.
    pub fn studio_version(&self) -> String {
        if let Some(id) = &self.studio_version_id {
            return id.clone();
        }
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
                check("DXVK Vulkan Layer", format!("OK ({})", self.dxvk_label()), LEVEL_OK)
            } else {
                check("DXVK Vulkan Layer", "Fetched on first launch".into(), LEVEL_WARN)
            },
            if self.studio_exe.is_some() {
                check("Prefix Integrity", "OK (Studio installed)".into(), LEVEL_OK)
            } else {
                check("Prefix Integrity", "Needs install".into(), LEVEL_WARN)
            },
            match &self.studio_theme {
                None => check("Studio Theme", "Off".into(), LEVEL_NEUTRAL),
                Some(studio_theme::Status::Ready) => check("Studio Theme", "OK (patched copy ready)".into(), LEVEL_OK),
                Some(studio_theme::Status::Pending) => {
                    check("Studio Theme", "Prepared on next launch".into(), LEVEL_OK)
                }
                Some(studio_theme::Status::Unsupported(e)) => {
                    check("Studio Theme", format!("Unsupported Studio build: {}", e), LEVEL_FAIL)
                }
            },
            if self.discord_socket.is_some() {
                check("Discord Rich Presence", "OK (Discord running)".into(), LEVEL_OK)
            } else {
                check("Discord Rich Presence", "Discord not running".into(), LEVEL_NEUTRAL)
            },
        ]
    }

    pub fn summary(&self) -> String {
        if self.studio_exe.is_none() {
            "Roblox Studio is not installed yet. Use Update (or Doctor → Install) to install it.".to_string()
        } else if self.wine_version.is_none() {
            "Wine was not found in PATH. Install Wine 9.0+ to launch Studio.".to_string()
        } else {
            format!("Ready • Roblox Studio {} • DXVK {}", self.studio_version(), self.dxvk_label())
        }
    }
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|content| content.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// One `## vX.Y.Z` section of CHANGELOG.md.
#[derive(Debug, PartialEq)]
pub(super) struct ChangelogEntry {
    pub version: String,
    pub items: Vec<String>,
}

/// The changelog of the installed release (`<root>/CHANGELOG.md`), or the bundled copy.
pub(super) fn load_changelog(root: &Path) -> Vec<ChangelogEntry> {
    let markdown = std::fs::read_to_string(root.join("CHANGELOG.md"));
    parse_changelog(markdown.as_deref().unwrap_or(BUNDLED_CHANGELOG))
}

/// Splits a Markdown changelog into `## vX.Y.Z` sections of `- item` bullets.
/// Text before the first section is ignored; `**bold**` markers are dropped and
/// non-bullet lines are appended to the previous item.
fn parse_changelog(markdown: &str) -> Vec<ChangelogEntry> {
    let mut entries: Vec<ChangelogEntry> = Vec::new();
    for line in markdown.lines().map(str::trim) {
        if let Some(heading) = line.strip_prefix("## ") {
            let version = heading.split_whitespace().next().unwrap_or(heading);
            entries.push(ChangelogEntry {
                version: version.to_string(),
                items: Vec::new(),
            });
            continue;
        }
        let Some(entry) = entries.last_mut() else { continue };
        if line.is_empty() {
            continue;
        }
        let text = line.replace("**", "");
        match text.strip_prefix("- ").or_else(|| text.strip_prefix("* ")) {
            Some(item) => entry.items.push(item.to_string()),
            None => match entry.items.last_mut() {
                Some(last) => {
                    last.push(' ');
                    last.push_str(&text);
                }
                None => entry.items.push(text),
            },
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\nIntro text.\n\n## v1.3.0 (2026-10-24)\n\n- **New UI**: rebuilt.\n- Faster\n  and wrapped.\n\n## v1.2.1\n\n- Old entry.\n";

    #[test]
    fn test_parse_changelog_sections() {
        assert_eq!(
            parse_changelog(SAMPLE),
            vec![
                ChangelogEntry {
                    version: "v1.3.0".into(),
                    items: vec!["New UI: rebuilt.".into(), "Faster and wrapped.".into()],
                },
                ChangelogEntry {
                    version: "v1.2.1".into(),
                    items: vec!["Old entry.".into()],
                },
            ]
        );
    }

    #[test]
    fn test_parse_changelog_without_sections() {
        assert!(parse_changelog("# Changelog\n\nNothing yet.").is_empty());
    }

    #[test]
    fn test_bundled_changelog_has_current_version() {
        let current = format!("v{}", env!("CARGO_PKG_VERSION"));
        assert!(parse_changelog(BUNDLED_CHANGELOG).iter().any(|e| e.version == current));
    }
}

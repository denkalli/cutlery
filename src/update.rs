//! Cutlery self-update from GitHub Releases and Roblox Studio version checks.
//! Shared by the Control Center (Update button) and `cutlery update`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::find_studio_exe;

const RELEASES_API: &str = "https://api.github.com/repos/denkalli/cutlery/releases/latest";
const STUDIO_VERSION_API: &str = "https://clientsettingscdn.roblox.com/v2/client-version/WindowsStudio64";

/// Written into each Studio version directory by scripts/install_studio.py.
pub(crate) const STUDIO_VERSION_MARKER: &str = "cutlery-studio-version.txt";

/// Release payload copied over the install root. `sandbox/` (prefix, DXVK) is never touched.
const PAYLOAD: &[&str] = &[
    "data",
    "scripts",
    "mcp",
    "assets",
    "plugins",
    "custom-libs",
    "CHANGELOG.md",
    "README.md",
    "LICENSE",
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Release {
    /// Tag, always `v`-prefixed (e.g. `v1.4.0`).
    pub version: String,
    pub asset_name: String,
    pub tarball_url: String,
    pub checksum_url: String,
}

/// Version of the Cutlery release installed at `root` (`v`-prefixed).
/// Reads the `version` file shipped with release packages, falling back to the compiled version.
pub(crate) fn installed_version(root: &Path) -> String {
    let exe_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
    let from_file = [Some(root.to_path_buf()), exe_dir]
        .into_iter()
        .flatten()
        .filter_map(|dir| fs::read_to_string(dir.join("version")).ok())
        .map(|content| content.trim().to_string())
        .find(|v| !v.is_empty());
    with_v_prefix(&from_file.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()))
}

fn with_v_prefix(version: &str) -> String {
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{}", version)
    }
}

pub(crate) fn curl_get(url: &str) -> Result<String, String> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "30", "-A", "Cutlery-Updater", url])
        .output()
        .map_err(|e| format!("curl is required for updates: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "Request to {} failed: {}",
            url,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn curl_download(url: &str, dest: &Path) -> Result<(), String> {
    let status = Command::new("curl")
        .args(["-fsSL", "-A", "Cutlery-Updater", url, "-o"])
        .arg(dest)
        .status()
        .map_err(|e| format!("curl is required for updates: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Download failed: {}", url))
    }
}

pub(crate) fn fetch_latest_release() -> Result<Release, String> {
    parse_release(&curl_get(RELEASES_API)?)
}

fn parse_release(json: &str) -> Result<Release, String> {
    let val: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("Invalid GitHub response: {}", e))?;
    let tag = val
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or("GitHub response has no tag_name")?;
    let version = with_v_prefix(tag);
    let asset_name = format!("cutlery-{}-linux-x86_64.tar.gz", version);
    let checksum_name = format!("{}.sha256", asset_name);

    let asset_url = |name: &str| {
        val.get("assets")?
            .as_array()?
            .iter()
            .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(name))?
            .get("browser_download_url")?
            .as_str()
            .map(str::to_string)
    };
    let tarball_url = asset_url(&asset_name).ok_or(format!("Release {} has no {}", version, asset_name))?;
    let checksum_url = asset_url(&checksum_name).ok_or(format!("Release {} has no {}", version, checksum_name))?;

    Ok(Release {
        version,
        asset_name,
        tarball_url,
        checksum_url,
    })
}

fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().trim_start_matches('v').split('.').map(|part| {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        digits.parse::<u64>().ok()
    });
    Some((parts.next()??, parts.next().unwrap_or(Some(0))?, parts.next().unwrap_or(Some(0))?))
}

/// True when `latest` is a strictly higher version than `current`.
pub(crate) fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(cur), Some(new)) => new > cur,
        _ => false,
    }
}

/// A source checkout must be updated with git, never overwritten by a release tarball.
pub(crate) fn is_source_checkout(root: &Path) -> bool {
    root.join("Cargo.toml").exists()
}

/// Why the install at `root` cannot replace itself with a release tarball, if it cannot.
pub(crate) fn self_update_blocker(root: &Path) -> Option<&'static str> {
    if is_source_checkout(root) {
        return Some("this is a source checkout, update it with git pull");
    }
    match crate::package_format() {
        Some("AppImage") => Some("download the new AppImage from GitHub Releases"),
        Some(_) => Some("install the new .snap from GitHub Releases"),
        None => None,
    }
}

fn cache_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir).join("cutlery"));
        }
    }
    let home = std::env::var("HOME").map_err(|_| "HOME is not set")?;
    Ok(PathBuf::from(home).join(".cache").join("cutlery"))
}

fn sha256_of(path: &Path) -> Result<String, String> {
    let out = Command::new("sha256sum")
        .arg(path)
        .output()
        .map_err(|e| format!("sha256sum is required for updates: {}", e))?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .map(str::to_lowercase)
        .ok_or_else(|| "sha256sum produced no output".to_string())
}

fn copy_recursively(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_recursively(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        fs::copy(src, dst)?;
    }
    Ok(())
}

/// Downloads, verifies (SHA-256) and installs `release` over the install at `root`.
/// Returns the path of the new `cutlery` binary.
pub(crate) fn apply_release(release: &Release, root: &Path) -> Result<PathBuf, String> {
    if let Some(reason) = self_update_blocker(root) {
        return Err(format!("Cutlery cannot update itself here: {}.", reason));
    }

    let work = cache_dir()?.join("updates");
    let _ = fs::remove_dir_all(&work);
    let staging = work.join("staging");
    fs::create_dir_all(&staging).map_err(|e| format!("Cannot create {}: {}", staging.display(), e))?;

    let tarball = work.join(&release.asset_name);
    curl_download(&release.tarball_url, &tarball)?;
    let expected = curl_get(&release.checksum_url)?
        .split_whitespace()
        .next()
        .map(str::to_lowercase)
        .unwrap_or_default();
    if expected.len() != 64 || sha256_of(&tarball)? != expected {
        let _ = fs::remove_dir_all(&work);
        return Err(format!("SHA-256 verification failed for {}; nothing was changed.", release.asset_name));
    }

    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&tarball)
        .arg("-C")
        .arg(&staging)
        .arg("--strip-components=1")
        .status()
        .map_err(|e| format!("tar failed: {}", e))?;
    let new_binary = staging.join("cutlery");
    if !status.success() || !new_binary.exists() {
        return Err(format!("{} is not a valid Cutlery package.", release.asset_name));
    }

    for item in PAYLOAD {
        let src = staging.join(item);
        if src.exists() {
            copy_recursively(&src, &root.join(item)).map_err(|e| format!("Cannot update {}: {}", item, e))?;
        }
    }
    fs::write(root.join("version"), format!("{}\n", release.version))
        .map_err(|e| format!("Cannot write version file: {}", e))?;

    // Swap the binary last; rename over a running executable is safe on Linux.
    let target = root.join("cutlery");
    let temp = root.join("cutlery.new");
    fs::copy(&new_binary, &temp).map_err(|e| format!("Cannot stage new binary: {}", e))?;
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temp, fs::Permissions::from_mode(0o755));
    }
    fs::rename(&temp, &target).map_err(|e| format!("Cannot replace {}: {}", target.display(), e))?;

    let _ = fs::remove_dir_all(&work);
    Ok(target)
}

/// Latest Roblox Studio build as (version GUID, human-readable version).
pub(crate) fn latest_studio_version() -> Result<(String, String), String> {
    let body = curl_get(STUDIO_VERSION_API)?;
    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Invalid Roblox response: {}", e))?;
    let field = |name: &str| val.get(name).and_then(|v| v.as_str()).map(str::to_string);
    match (field("clientVersionUpload"), field("version")) {
        (Some(guid), Some(version)) => Ok((guid, version)),
        _ => Err("Roblox version API returned an unexpected response".to_string()),
    }
}

/// Version GUID (directory name) of the installed Studio, if any.
pub(crate) fn installed_studio_guid(prefix: &Path) -> Option<String> {
    let exe = find_studio_exe(prefix)?;
    Some(exe.parent()?.file_name()?.to_string_lossy().to_string())
}

pub(crate) enum StudioCheck {
    /// The installed build is the latest one (its version).
    UpToDate(String),
    /// Missing or older than Roblox's latest build (latest version, if known).
    NeedsInstall(Option<String>),
    /// Studio is installed but the latest version could not be fetched.
    Unknown(String),
}

pub(crate) fn check_studio(prefix: &Path) -> StudioCheck {
    let installed = installed_studio_guid(prefix);
    match (latest_studio_version(), installed) {
        (Ok((guid, version)), Some(current)) if guid == current => StudioCheck::UpToDate(version),
        (Ok((_, version)), _) => StudioCheck::NeedsInstall(Some(version)),
        (Err(_), None) => StudioCheck::NeedsInstall(None),
        (Err(e), Some(_)) => StudioCheck::Unknown(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("v1.3.0", "v1.4.0"));
        assert!(is_newer("1.3.0", "v1.10.0"));
        assert!(is_newer("v1.3.9", "v2.0.0"));
        assert!(!is_newer("v1.3.0", "v1.3.0"));
        assert!(!is_newer("v1.4.0", "v1.3.2"));
        assert!(!is_newer("v1.3.0", "garbage"));
        assert!(is_newer("v1.3", "v1.3.1"));
    }

    #[test]
    fn test_parse_release_picks_matching_assets() {
        let json = r#"{
            "tag_name": "v1.4.0",
            "assets": [
                {"name": "other.zip", "browser_download_url": "https://x/other.zip"},
                {"name": "cutlery-v1.4.0-linux-x86_64.tar.gz", "browser_download_url": "https://x/c.tar.gz"},
                {"name": "cutlery-v1.4.0-linux-x86_64.tar.gz.sha256", "browser_download_url": "https://x/c.sha256"}
            ]
        }"#;
        assert_eq!(
            parse_release(json).unwrap(),
            Release {
                version: "v1.4.0".into(),
                asset_name: "cutlery-v1.4.0-linux-x86_64.tar.gz".into(),
                tarball_url: "https://x/c.tar.gz".into(),
                checksum_url: "https://x/c.sha256".into(),
            }
        );
    }

    #[test]
    fn test_parse_release_missing_asset() {
        assert!(parse_release(r#"{"tag_name": "v1.4.0", "assets": []}"#).is_err());
        assert!(parse_release("not json").is_err());
    }

    #[test]
    fn test_source_checkout_is_never_overwritten() {
        let release = Release {
            version: "v9.9.9".into(),
            asset_name: "x.tar.gz".into(),
            tarball_url: "https://invalid.example/x".into(),
            checksum_url: "https://invalid.example/x.sha256".into(),
        };
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(apply_release(&release, root).unwrap_err().contains("source checkout"));
    }
}

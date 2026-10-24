//! Roblox Studio theming (findings in docs/studio-themes.md).
//!
//! Studio reads its Qt UI colors from two JSON files compiled into the exe as Qt resources.
//! Cutlery writes a copy of the exe whose two theme paths point at `./Platform/...` instead of
//! `:/Platform/...`, extracts the original JSON next to it, and writes theme files remapped to
//! the selected Cutlery theme. The original `RobloxStudioBeta.exe` is never modified.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::AppearanceConfig;
use crate::theme::{self, Rgba, ThemeDef};

/// Patched copy written next to `RobloxStudioBeta.exe`.
pub const PATCHED_EXE: &str = "RobloxStudioCutlery.exe";
/// Theme folder, relative to the Studio version folder (Studio's working directory).
pub const THEME_DIR: &str = "Platform/Base/QtUI/themes";
const STAMP_FILE: &str = "RobloxStudioCutlery.stamp";

const RESOURCE_DARK: &[u8] = b":/Platform/Base/QtUI/themes/FoundationDarkTheme.json";
const RESOURCE_LIGHT: &[u8] = b":/Platform/Base/QtUI/themes/FoundationLightTheme.json";

/// Script Editor preset keys that are syntax colors; they keep Studio's values unless the
/// theme sets them in `[studio.syntax]`. Every other preset key (background, selection,
/// active line, menus…) follows the UI palette.
const SYNTAX_KEYS: &[&str] = &[
    "Default", "Operator", "Number", "String", "Comment", "Keyword", "Builtin", "Method", "Property", "Nil", "Bool",
    "Function", "Local", "Self", "LuauKeyword", "FunctionName", "TODO", "Type", "Bracket", "Warning", "Error", "Hint",
    "Info", "FindSelectionBackground", "ReviewableChangeAddedText", "ReviewableChangeRemovedText",
];

/// Where a Studio color goes: a Cutlery theme color, the accent, or a blend.
#[derive(Clone, Copy)]
enum Target {
    Key(&'static str),
    /// `pct`% of the first color, the rest of the second. `"accent"` names the accent.
    Mix(&'static str, &'static str, u8),
}
use Target::{Key, Mix};

/// Dominant colors of `FoundationDarkTheme.json` and their Cutlery roles.
const DARK_ROLES: &[(&str, Target)] = &[
    ("121215", Mix("surface-sunken", "window-border", 70)),
    ("191A1F", Key("surface-sunken")),
    ("202227", Key("window-bg")),
    ("272930", Key("surface")),
    ("282931", Key("surface")),
    ("2E2E38", Key("surface-hover")),
    ("353741", Key("border")),
    ("494D5A", Key("border-strong")),
    ("6A6F81", Key("text-faint")),
    ("BCBEC8", Mix("text", "text-muted", 50)),
    ("D5D7DD", Key("text")),
    ("E6E7EA", Key("text")),
    ("F7F7F8", Key("text")),
    ("335FFF", Key("accent")),
    ("528BFF", Mix("accent", "text", 80)),
    ("002DD6", Mix("accent", "window-bg", 75)),
    ("0027B8", Mix("accent", "window-bg", 55)),
    ("002299", Mix("accent", "window-bg", 55)),
    ("13235D", Mix("accent", "window-bg", 30)),
    ("E75750", Key("danger")),
    ("DF281F", Key("danger")),
    ("EB7973", Mix("danger", "text", 80)),
    ("F57630", Key("warn")),
    ("F2BA2A", Key("warn")),
    ("1DB16A", Key("ok")),
];

/// Dominant colors of `FoundationLightTheme.json` and their Cutlery roles.
const LIGHT_ROLES: &[(&str, Target)] = &[
    ("FFFFFF", Key("surface")),
    ("F7F7F8", Key("window-bg")),
    ("EEEFF1", Key("surface-sunken")),
    ("E6E7EA", Mix("surface-sunken", "border", 70)),
    ("D5D7DD", Key("border")),
    ("BCBEC8", Key("border-strong")),
    ("6A6F81", Key("text-muted")),
    ("494D5A", Key("text-muted")),
    ("272930", Key("text")),
    ("202227", Key("text")),
    ("191A1F", Key("text")),
    ("121215", Key("text")),
    ("000000", Key("text")),
    ("335FFF", Key("accent")),
    ("528BFF", Mix("accent", "surface", 80)),
    ("70A0FF", Mix("accent", "surface", 65)),
    ("8FB4FF", Mix("accent", "surface", 50)),
    ("ADC9FF", Mix("accent", "surface", 35)),
    ("D0DAFE", Mix("accent", "surface", 22)),
    ("CCDDFF", Mix("accent", "surface", 22)),
    ("EBF1FF", Mix("accent", "surface", 10)),
    ("002DD6", Mix("accent", "text", 80)),
    ("0027B8", Mix("accent", "text", 70)),
    ("A41109", Key("danger")),
    ("DF281F", Key("danger")),
    ("AE800B", Key("warn")),
    ("815D00", Key("warn")),
    ("01854B", Key("ok")),
    ("004A29", Key("ok")),
];

/// The two original theme files, as compiled into `RobloxStudioBeta.exe`.
#[derive(Debug, Clone, PartialEq)]
pub struct Originals {
    pub dark: String,
    pub light: String,
}

fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    let mut hits = Vec::new();
    let mut start = 0;
    while let Some(pos) = haystack[start..].windows(needle.len()).position(|w| w == needle) {
        hits.push(start + pos);
        start += pos + 1;
    }
    hits
}

fn be32(bytes: &[u8]) -> usize {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize
}

/// Finds the two theme files among the exe's Qt resources. Compressed resources are stored as
/// `[u32 BE stored size][u32 BE uncompressed size][zlib stream]`.
pub fn extract_originals(exe: &[u8]) -> Result<Originals, String> {
    let (mut dark, mut light) = (None, None);
    for i in 8..exe.len().saturating_sub(2) {
        if exe[i] != 0x78 || !matches!(exe[i + 1], 0x01 | 0x5E | 0x9C | 0xDA) {
            continue;
        }
        let (stored, size) = (be32(&exe[i - 8..i - 4]), be32(&exe[i - 4..i]));
        if !(10_000..=4_000_000).contains(&size) || stored <= 4 || stored - 4 > size || i + stored - 4 > exe.len() {
            continue;
        }
        let mut text = String::new();
        let decoded = flate2::read::ZlibDecoder::new(&exe[i..i + stored - 4]).read_to_string(&mut text);
        if decoded.is_err() || text.len() != size || !text.trim_start().starts_with('{') {
            continue;
        }
        let Ok(json) = serde_json::from_str::<Value>(&text) else { continue };
        if json.get("Colors").is_none() || json.get("Presets").is_none() {
            continue;
        }
        match json.get("Name").and_then(Value::as_str) {
            Some("Dark") => dark = Some(text),
            Some("Light") => light = Some(text),
            _ => {}
        }
        if dark.is_some() && light.is_some() {
            break;
        }
    }
    match (dark, light) {
        (Some(dark), Some(light)) => Ok(Originals { dark, light }),
        _ => Err("the Foundation theme files were not found in RobloxStudioBeta.exe".into()),
    }
}

/// Returns a copy of the exe whose two theme resource paths load from the working directory.
/// Refuses anything but exactly one occurrence of each path, so a changed Studio build fails
/// loudly instead of being patched blindly. Only the leading `:` of each path changes.
pub fn patch_exe(exe: &[u8]) -> Result<Vec<u8>, String> {
    let mut patched = exe.to_vec();
    for path in [RESOURCE_DARK, RESOURCE_LIGHT] {
        let hits = find_all(exe, path);
        if hits.len() != 1 {
            return Err(format!(
                "expected one \"{}\" in RobloxStudioBeta.exe, found {}",
                String::from_utf8_lossy(path),
                hits.len()
            ));
        }
        patched[hits[0]] = b'.';
    }
    Ok(patched)
}

fn mix(a: Rgba, b: Rgba, pct_a: u8) -> Rgba {
    let blend = |x: u8, y: u8| ((x as u32 * pct_a as u32 + y as u32 * (100 - pct_a as u32) + 50) / 100) as u8;
    Rgba { r: blend(a.r, b.r), g: blend(a.g, b.g), b: blend(a.b, b.b), a: 255 }
}

/// Resolves the remap table (original `RRGGBB` → new color) for a theme on one Studio base.
fn palette(dark: bool, theme: &ThemeDef, accent: Rgba) -> BTreeMap<String, Rgba> {
    let color = |key: &str| if key == "accent" { accent } else { theme.color(key) };
    let roles = if dark { DARK_ROLES } else { LIGHT_ROLES };
    let mut map: BTreeMap<String, Rgba> = roles
        .iter()
        .map(|(from, target)| {
            let to = match *target {
                Key(key) => color(key),
                Mix(a, b, pct) => mix(color(a), color(b), pct),
            };
            (from.to_string(), to)
        })
        .collect();
    map.extend(theme.studio.remap.clone());
    map
}

fn hex(c: Rgba) -> String {
    if c.a == 255 {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", c.a, c.r, c.g, c.b)
    }
}

/// Remaps one Studio color string; `#AARRGGBB` keeps its alpha. Non-colors return `None`.
fn remap_color(value: &str, map: &BTreeMap<String, Rgba>) -> Option<String> {
    let digits = value.strip_prefix('#')?;
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let (alpha, rgb) = match digits.len() {
        6 => (None, digits),
        8 => (Some(&digits[..2]), &digits[2..]),
        _ => return None,
    };
    let to = map.get(&rgb.to_ascii_uppercase())?;
    Some(match alpha {
        Some(alpha) => format!("#{}{:02X}{:02X}{:02X}", alpha.to_ascii_uppercase(), to.r, to.g, to.b),
        None => hex(*to),
    })
}

fn remap_value(value: &mut Value, map: &BTreeMap<String, Rgba>) {
    match value {
        Value::String(s) => {
            if let Some(new) = remap_color(s, map) {
                *s = new;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| remap_value(v, map)),
        Value::Object(fields) => fields.values_mut().for_each(|v| remap_value(v, map)),
        _ => {}
    }
}

/// Renders `original` (one of `Originals`) for `theme`. Returns the JSON plus a warning for
/// every `[studio.colors]` / `[studio.syntax]` name this Studio build does not have.
pub fn render(original: &str, theme: &ThemeDef, accent: Rgba) -> Result<(String, Vec<String>), String> {
    let mut json: Value = serde_json::from_str(original).map_err(|e| format!("invalid Studio theme JSON: {}", e))?;
    let dark = json.get("Name").and_then(Value::as_str) != Some("Light");
    let map = palette(dark, theme, accent);
    let mut warnings = Vec::new();

    let root = json.as_object_mut().ok_or("Studio theme JSON is not an object")?;
    for (key, value) in root.iter_mut() {
        if key != "Presets" {
            remap_value(value, &map);
        }
    }

    if let Some(presets) = root.get_mut("Presets").and_then(Value::as_object_mut) {
        for preset in presets.values_mut().filter_map(Value::as_object_mut) {
            for (key, value) in preset.iter_mut() {
                if !SYNTAX_KEYS.contains(&key.as_str()) {
                    remap_value(value, &map);
                }
            }
        }
        for (key, color) in &theme.studio.syntax {
            let mut found = false;
            for preset in presets.values_mut().filter_map(Value::as_object_mut) {
                if let Some(slot) = preset.get_mut(key) {
                    *slot = Value::String(hex(*color));
                    found = true;
                }
            }
            if !found {
                warnings.push(format!("studio.syntax: Studio has no \"{}\" color", key));
            }
        }
    }

    if let Some(colors) = root.get_mut("Colors").and_then(Value::as_array_mut) {
        for (name, color) in &theme.studio.colors {
            let (style, modifier) = name.split_once('.').unwrap_or((name, "Default"));
            let entry = colors
                .iter_mut()
                .filter_map(Value::as_object_mut)
                .find_map(|entry| entry.get_mut(style))
                .and_then(Value::as_object_mut);
            match entry {
                Some(entry) => {
                    entry.insert(modifier.to_string(), Value::String(hex(*color)));
                }
                None => warnings.push(format!("studio.colors: Studio has no \"{}\" color", style)),
            }
        }
    }

    let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    Ok((text, warnings))
}

/// Size and modification time of the original exe; a change means Studio was updated in place.
fn stamp_of(exe: &Path) -> Result<String, String> {
    let meta = fs::metadata(exe).map_err(|e| format!("{}: {}", exe.display(), e))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Ok(format!("{} {}", meta.len(), modified))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)
        .and_then(|_| fs::rename(&tmp, path))
        .map_err(|e| format!("{}: {}", path.display(), e))
}

fn original_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("Foundation{}Theme.orig.json", name))
}

/// The cached originals, when the patched exe and its stamp match the current Studio exe.
fn cached_originals(studio_dir: &Path, stamp: &str) -> Option<Originals> {
    let theme_dir = studio_dir.join(THEME_DIR);
    let up_to_date = studio_dir.join(PATCHED_EXE).is_file()
        && fs::read_to_string(studio_dir.join(STAMP_FILE)).is_ok_and(|s| s.trim() == stamp);
    if !up_to_date {
        return None;
    }
    let read = |name| fs::read_to_string(original_path(&theme_dir, name)).ok();
    Some(Originals { dark: read("Dark")?, light: read("Light")? })
}

/// Makes sure the patched exe and the extracted originals in `studio_dir` match the current
/// `RobloxStudioBeta.exe`, redoing both after a Studio update. Returns the originals.
pub fn ensure_patched(studio_dir: &Path) -> Result<Originals, String> {
    let exe = studio_dir.join("RobloxStudioBeta.exe");
    let stamp = stamp_of(&exe)?;
    let theme_dir = studio_dir.join(THEME_DIR);
    let stamp_path = studio_dir.join(STAMP_FILE);
    if let Some(originals) = cached_originals(studio_dir, &stamp) {
        return Ok(originals);
    }

    let bytes = fs::read(&exe).map_err(|e| format!("{}: {}", exe.display(), e))?;
    let patched = patch_exe(&bytes)?;
    let originals = extract_originals(&bytes)?;
    fs::create_dir_all(&theme_dir).map_err(|e| format!("{}: {}", theme_dir.display(), e))?;
    write_atomic(&original_path(&theme_dir, "Dark"), originals.dark.as_bytes())?;
    write_atomic(&original_path(&theme_dir, "Light"), originals.light.as_bytes())?;
    write_atomic(&studio_dir.join(PATCHED_EXE), &patched)?;
    write_atomic(&stamp_path, stamp.as_bytes())?;
    Ok(originals)
}

/// Studio theme readiness, shown on the Doctor page.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Patched copy and originals match the installed Studio.
    Ready,
    /// This Studio build can be themed; it is prepared on the next themed launch.
    Pending,
    /// This Studio build cannot be themed (for example, Roblox changed the theme files).
    Unsupported(String),
}

/// Checks `studio_dir` without writing anything. Reads the whole exe unless it is already prepared.
pub fn status(studio_dir: &Path) -> Status {
    let exe = studio_dir.join("RobloxStudioBeta.exe");
    let stamp = match stamp_of(&exe) {
        Ok(stamp) => stamp,
        Err(e) => return Status::Unsupported(e),
    };
    if cached_originals(studio_dir, &stamp).is_some() {
        return Status::Ready;
    }
    let checked = fs::read(&exe)
        .map_err(|e| format!("{}: {}", exe.display(), e))
        .and_then(|bytes| patch_exe(&bytes).and_then(|_| extract_originals(&bytes)));
    match checked {
        Ok(_) => Status::Pending,
        Err(e) => Status::Unsupported(e),
    }
}

/// Writes both theme files Studio now loads from disk: the theme's base gets the rendered
/// theme, the other one Studio's original (both paths must exist once the exe is patched).
pub fn write_theme(studio_dir: &Path, originals: &Originals, theme: &ThemeDef, accent: Rgba) -> Result<Vec<String>, String> {
    let theme_dir = studio_dir.join(THEME_DIR);
    let (themed, other, themed_name, other_name) = if theme.dark {
        (&originals.dark, &originals.light, "Dark", "Light")
    } else {
        (&originals.light, &originals.dark, "Light", "Dark")
    };
    let (json, warnings) = render(themed, theme, accent)?;
    fs::create_dir_all(&theme_dir).map_err(|e| format!("{}: {}", theme_dir.display(), e))?;
    write_atomic(&theme_dir.join(format!("Foundation{}Theme.json", themed_name)), json.as_bytes())?;
    write_atomic(&theme_dir.join(format!("Foundation{}Theme.json", other_name)), other.as_bytes())?;
    Ok(warnings)
}

/// Result of preparing a themed launch.
pub struct Applied {
    /// Exe to launch instead of `RobloxStudioBeta.exe`, with `studio_dir` as working directory.
    pub exe: PathBuf,
    pub theme_name: String,
    /// Whether the theme is dark: Studio must use its matching Dark/Light setting.
    pub dark: bool,
    /// Theme load problems and `[studio]` names this Studio build does not know.
    pub warnings: Vec<String>,
}

/// Prepares `studio_dir` to launch with the configured Cutlery theme. An unknown theme id
/// falls back to the default theme, like the Control Center does.
pub fn apply(studio_dir: &Path, appearance: &AppearanceConfig) -> Result<Applied, String> {
    let (themes, mut warnings) = theme::load_all(&theme::user_dir());
    let selected = themes
        .iter()
        .find(|t| t.id == appearance.theme)
        .or_else(|| {
            warnings.push(format!("theme \"{}\" not found, using {}", appearance.theme, theme::DEFAULT_THEME));
            themes.iter().find(|t| t.id == theme::DEFAULT_THEME)
        })
        .ok_or("no themes available")?;
    let accent = selected.accent.unwrap_or_else(|| theme::accent_by_name(&appearance.accent_color));

    let originals = ensure_patched(studio_dir)?;
    warnings.extend(write_theme(studio_dir, &originals, selected, accent)?);
    Ok(Applied {
        exe: studio_dir.join(PATCHED_EXE),
        theme_name: selected.name.clone(),
        dark: selected.dark,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{self, parse_color};
    use std::io::Write;

    const SAMPLE: &str = r##"{
        "Name": "Dark",
        "Presets": {
            "RobloxDefault": { "Keyword": "#EB7973", "Background": "#202227", "ActiveLine": "#353741" },
            "Custom": { "Keyword": "#EB7973", "Background": "#202227" }
        },
        "Colors": [
            { "MainBackground": { "Default": "#191A1F" } },
            { "Item": { "Default": "#191A1F", "Hover": "#353741" } }
        ],
        "Ribbon": { "background": "#191A1F", "tab-topbar": "#528BFF" },
        "Package": { "ScrollBarHandle": { "default": "#1ECCCCCC" }, "Shadow": "#80202227" },
        "Paths": [ { "Icon": { "Default": ":/images/icon.png" } } ],
        "Merge": { "Text": "#ABCDEF", "Bad": "#00000" }
    }"##;

    fn dark_theme() -> ThemeDef {
        theme::load_all(Path::new("/nonexistent")).0.into_iter().find(|t| t.id == "cutlery-dark").unwrap()
    }

    fn compress(text: &str) -> Vec<u8> {
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(text.as_bytes()).unwrap();
        enc.finish().unwrap()
    }

    /// A fake exe holding both theme paths and both compressed resources.
    fn fake_exe(dark: &str, light: &str) -> Vec<u8> {
        let mut exe = b"MZ junk ".to_vec();
        exe.extend_from_slice(RESOURCE_DARK);
        exe.extend_from_slice(b"\0padding\0");
        exe.extend_from_slice(RESOURCE_LIGHT);
        for text in [dark, light] {
            let z = compress(text);
            exe.extend_from_slice(&((z.len() + 4) as u32).to_be_bytes());
            exe.extend_from_slice(&(text.len() as u32).to_be_bytes());
            exe.extend_from_slice(&z);
            exe.extend_from_slice(b"\0\0trailer");
        }
        exe
    }

    fn padded(name: &str) -> String {
        // Real theme files are ~50 KB; extract_originals ignores resources under 10 KB.
        format!("{{\"Name\": \"{}\", \"Presets\": {{}}, \"Colors\": [], \"Pad\": \"{}\"}}", name, "x".repeat(12_000))
    }

    #[test]
    fn test_patch_exe_changes_two_bytes() {
        let exe = fake_exe(&padded("Dark"), &padded("Light"));
        let patched = patch_exe(&exe).unwrap();
        assert_eq!(patched.len(), exe.len());
        assert_eq!(exe.iter().zip(&patched).filter(|(a, b)| a != b).count(), 2);
        assert_eq!(find_all(&patched, b"./Platform/Base/QtUI/themes/Foundation").len(), 2);
        // Already patched, missing or duplicated paths are refused.
        assert!(patch_exe(&patched).is_err());
        assert!(patch_exe(b"no theme paths here").is_err());
        let mut doubled = exe.clone();
        doubled.extend_from_slice(RESOURCE_DARK);
        assert!(patch_exe(&doubled).is_err());
    }

    #[test]
    fn test_extract_originals() {
        let (dark, light) = (padded("Dark"), padded("Light"));
        let originals = extract_originals(&fake_exe(&dark, &light)).unwrap();
        assert_eq!(originals, Originals { dark, light });
        assert!(extract_originals(b"nothing").is_err());
    }

    #[test]
    fn test_render_remaps_palette() {
        let theme = dark_theme();
        let accent = parse_color("#ff0000").unwrap();
        let (json, warnings) = render(SAMPLE, &theme, accent).unwrap();
        assert!(warnings.is_empty());
        let v: Value = serde_json::from_str(&json).unwrap();
        let sunken = hex(theme.color("surface-sunken"));
        assert_eq!(v["Colors"][0]["MainBackground"]["Default"], sunken.as_str());
        assert_eq!(v["Ribbon"]["background"], sunken.as_str());
        // #AARRGGBB keeps its alpha.
        let bg = theme.color("window-bg");
        assert_eq!(v["Package"]["Shadow"], format!("#80{:02X}{:02X}{:02X}", bg.r, bg.g, bg.b).as_str());
        // Accent-derived colors.
        assert_eq!(v["Ribbon"]["tab-topbar"], hex(mix(accent, theme.color("text"), 80)).as_str());
        // Unknown colors, invalid colors and non-colors are untouched.
        assert_eq!(v["Merge"]["Text"], "#ABCDEF");
        assert_eq!(v["Merge"]["Bad"], "#00000");
        assert_eq!(v["Package"]["ScrollBarHandle"]["default"], "#1ECCCCCC");
        assert_eq!(v["Paths"][0]["Icon"]["Default"], ":/images/icon.png");
        // Presets: UI keys follow the theme, syntax keys keep Studio's colors.
        assert_eq!(v["Presets"]["RobloxDefault"]["Background"], hex(bg).as_str());
        assert_eq!(v["Presets"]["RobloxDefault"]["Keyword"], "#EB7973");
        // The key order of the original file is kept.
        assert!(json.find("\"Name\"").unwrap() < json.find("\"Presets\"").unwrap());
    }

    #[test]
    fn test_render_applies_studio_overrides() {
        let mut theme = dark_theme();
        theme.studio.remap.insert("191A1F".into(), parse_color("#010203").unwrap());
        theme.studio.colors.insert("Item.Hover".into(), parse_color("#0a0b0c").unwrap());
        theme.studio.colors.insert("Nope".into(), parse_color("#000").unwrap());
        theme.studio.syntax.insert("Keyword".into(), parse_color("#ff79c6").unwrap());
        theme.studio.syntax.insert("Nope".into(), parse_color("#000").unwrap());
        let (json, warnings) = render(SAMPLE, &theme, parse_color("#ff0000").unwrap()).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["Colors"][0]["MainBackground"]["Default"], "#010203");
        assert_eq!(v["Colors"][1]["Item"]["Hover"], "#0A0B0C");
        assert_eq!(v["Presets"]["RobloxDefault"]["Keyword"], "#FF79C6");
        assert_eq!(v["Presets"]["Custom"]["Keyword"], "#FF79C6");
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn test_ensure_patched_and_write_theme() {
        let dir = std::env::temp_dir().join(format!("cutlery-studio-theme-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (dark, light) = (padded("Dark"), padded("Light"));
        fs::write(dir.join("RobloxStudioBeta.exe"), fake_exe(&dark, &light)).unwrap();

        assert_eq!(status(&dir), Status::Pending);
        let originals = ensure_patched(&dir).unwrap();
        assert_eq!(originals.dark, dark);
        assert_eq!(status(&dir), Status::Ready);
        assert!(dir.join(PATCHED_EXE).is_file());
        // Second call reuses the stamp and cached originals.
        assert_eq!(ensure_patched(&dir).unwrap(), originals);

        let light_theme = theme::load_all(Path::new("/nonexistent")).0.into_iter().find(|t| !t.dark).unwrap();
        write_theme(&dir, &originals, &light_theme, parse_color("#3b82f6").unwrap()).unwrap();
        let themes = dir.join(THEME_DIR);
        assert_eq!(fs::read_to_string(themes.join("FoundationDarkTheme.json")).unwrap(), dark);
        assert!(fs::read_to_string(themes.join("FoundationLightTheme.json")).unwrap().contains("\"Light\""));
        fs::write(dir.join("RobloxStudioBeta.exe"), b"updated Studio without theme paths").unwrap();
        assert!(matches!(status(&dir), Status::Unsupported(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Runs against the Studio installed in the local sandbox, when there is one:
    /// `cargo test -- --ignored real_studio`.
    #[test]
    #[ignore]
    fn test_real_studio_exe() {
        let root = std::env::var("CARGO_MANIFEST_DIR").map(PathBuf::from).unwrap();
        let Some(exe) = crate::find_studio_exe(&root.join("sandbox").join("prefix")) else { return };
        let bytes = fs::read(&exe).unwrap();
        assert_eq!(patch_exe(&bytes).unwrap().len(), bytes.len());
        let originals = extract_originals(&bytes).unwrap();
        for theme in theme::load_all(Path::new("/nonexistent")).0 {
            for original in [&originals.dark, &originals.light] {
                let (json, warnings) = render(original, &theme, theme::accent_by_name("blue")).unwrap();
                assert!(warnings.is_empty(), "{}: {:?}", theme.id, warnings);
                serde_json::from_str::<Value>(&json).unwrap();
            }
        }
    }
}

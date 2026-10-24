//! Control Center color themes: built-in TOML files under data/themes/ plus user files in
//! `~/.config/cutlery/themes/`. A theme sets any subset of `KEYS`; the rest is inherited
//! from its base palette (`cutlery-dark` or `cutlery-light`).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::CutleryConfig;

/// Every color a theme can set (except the optional `accent`), named like the Slint `Theme` tokens.
pub const KEYS: &[&str] = &[
    "window-bg",
    "window-border",
    "surface",
    "surface-hover",
    "surface-sunken",
    "overlay-bg",
    "border",
    "border-strong",
    "scrim",
    "shadow",
    "hover-tint",
    "status-glass",
    "card-glass",
    "sidebar-glass",
    "hero-top",
    "hero-bottom",
    "text",
    "text-soft",
    "text-muted",
    "text-faint",
    "ok",
    "info",
    "warn",
    "danger",
];

pub const DEFAULT_THEME: &str = "cutlery-dark";

const BUILTIN: &[(&str, &str)] = &[
    ("cutlery-dark", include_str!("../data/themes/cutlery-dark.toml")),
    ("cutlery-light", include_str!("../data/themes/cutlery-light.toml")),
    ("midnight", include_str!("../data/themes/midnight.toml")),
    ("oled", include_str!("../data/themes/oled.toml")),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Accent chips offered when a theme does not fix its accent; mirrors `Theme.accent-for` in ui/theme.slint.
pub fn accent_by_name(name: &str) -> Rgba {
    let hex = match name {
        "cyan" => "#0ea5e9",
        "indigo" => "#6366f1",
        "emerald" => "#10b981",
        "amber" => "#f59e0b",
        "violet" => "#8b5cf6",
        "rose" => "#f43f5e",
        _ => "#3b82f6",
    };
    parse_color(hex).unwrap_or(Rgba { r: 59, g: 130, b: 246, a: 255 })
}

/// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`.
pub fn parse_color(value: &str) -> Result<Rgba, String> {
    let hex = value.trim().strip_prefix('#').ok_or_else(|| format!("'{}' must start with #", value))?;
    let digits: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()
        .ok_or_else(|| format!("'{}' is not a hex color", value))?;
    let pair = |i: usize| digits[i] * 16 + digits[i + 1];
    match digits.len() {
        3 => Ok(Rgba { r: digits[0] * 17, g: digits[1] * 17, b: digits[2] * 17, a: 255 }),
        6 => Ok(Rgba { r: pair(0), g: pair(2), b: pair(4), a: 255 }),
        8 => Ok(Rgba { r: pair(0), g: pair(2), b: pair(4), a: pair(6) }),
        _ => Err(format!("'{}' must have 3, 6 or 8 hex digits", value)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default = "default_base")]
    base: String,
    /// Home page artwork, relative to the theme file (user themes only).
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(default)]
    studio: StudioFile,
}

/// Optional `[studio]` table: fine-tuning for the Roblox Studio theme (see src/studio_theme.rs).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct StudioFile {
    /// Original Studio color (`#RRGGBB`) to its replacement.
    remap: BTreeMap<String, String>,
    /// `StudioStyleGuideColor` name (`MainBackground`, or `MainBackground.Hover` for a modifier).
    colors: BTreeMap<String, String>,
    /// Script Editor preset keys (`Keyword`, `String`, `Background`…), applied to every preset.
    syntax: BTreeMap<String, String>,
}

/// Parsed `[studio]` table; colors are validated, names are checked against Studio at render time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StudioOverrides {
    /// Keyed by the original color as uppercase `RRGGBB` (no `#`).
    pub remap: BTreeMap<String, Rgba>,
    pub colors: BTreeMap<String, Rgba>,
    pub syntax: BTreeMap<String, Rgba>,
}

fn default_base() -> String {
    "dark".to_string()
}

/// A fully resolved theme: `colors` holds every key in `KEYS`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDef {
    pub id: String,
    pub name: String,
    pub author: String,
    pub dark: bool,
    pub colors: BTreeMap<String, Rgba>,
    /// Fixed accent; `None` lets the user pick one of the accent chips.
    pub accent: Option<Rgba>,
    pub background: Option<PathBuf>,
    pub studio: StudioOverrides,
}

impl ThemeDef {
    pub fn color(&self, key: &str) -> Rgba {
        self.colors.get(key).copied().unwrap_or(Rgba { r: 255, g: 0, b: 255, a: 255 })
    }
}

fn base_palette(dark: bool) -> BTreeMap<String, Rgba> {
    let id = if dark { "cutlery-dark" } else { "cutlery-light" };
    let source = BUILTIN.iter().find(|(name, _)| *name == id).map(|(_, s)| *s).unwrap_or_default();
    parse_theme(id, source, None, &BTreeMap::new()).map(|t| t.colors).unwrap_or_default()
}

/// Parses one theme file on top of `base` (empty for the base palettes themselves).
fn parse_theme(id: &str, source: &str, dir: Option<&Path>, base: &BTreeMap<String, Rgba>) -> Result<ThemeDef, String> {
    let file: ThemeFile = toml::from_str(source).map_err(|e| e.message().to_string())?;
    let dark = match file.base.as_str() {
        "dark" => true,
        "light" => false,
        other => return Err(format!("base must be \"dark\" or \"light\", not \"{}\"", other)),
    };

    let mut colors = base.clone();
    let mut accent = None;
    for (key, value) in &file.colors {
        let color = parse_color(value).map_err(|e| format!("{}: {}", key, e))?;
        if key == "accent" {
            accent = Some(color);
        } else if KEYS.contains(&key.as_str()) {
            colors.insert(key.clone(), color);
        } else {
            return Err(format!("unknown color \"{}\"", key));
        }
    }
    if let Some(missing) = KEYS.iter().find(|k| !colors.contains_key(**k)) {
        return Err(format!("missing color \"{}\"", missing));
    }

    let parse_map = |section: &str, map: &BTreeMap<String, String>| -> Result<BTreeMap<String, Rgba>, String> {
        map.iter()
            .map(|(k, v)| parse_color(v).map(|c| (k.clone(), c)).map_err(|e| format!("studio.{}.{}: {}", section, k, e)))
            .collect()
    };
    let studio = StudioOverrides {
        remap: parse_map("remap", &file.studio.remap)?
            .into_iter()
            .map(|(from, to)| match parse_color(&from) {
                Ok(c) if c.a == 255 => Ok((format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b), to)),
                _ => Err(format!("studio.remap: \"{}\" must be an opaque #rrggbb color", from)),
            })
            .collect::<Result<_, _>>()?,
        colors: parse_map("colors", &file.studio.colors)?,
        syntax: parse_map("syntax", &file.studio.syntax)?,
    };

    let background = match (file.background, dir) {
        (Some(rel), Some(dir)) => {
            let path = dir.join(rel);
            if !path.is_file() {
                return Err(format!("background image not found: {}", path.display()));
            }
            Some(path)
        }
        _ => None,
    };

    Ok(ThemeDef {
        id: id.to_string(),
        name: file.name,
        author: file.author,
        dark,
        colors,
        accent,
        background,
        studio,
    })
}

/// Parses a theme that inherits the missing colors from its base palette.
fn parse_with_base(id: &str, source: &str, dir: Option<&Path>) -> Result<ThemeDef, String> {
    let dark = toml::from_str::<ThemeFile>(source)
        .map(|f| f.base != "light")
        .unwrap_or(true);
    parse_theme(id, source, dir, &base_palette(dark))
}

/// Folder for user themes, next to config.toml.
pub fn user_dir() -> PathBuf {
    CutleryConfig::config_path()
        .parent()
        .map(|dir| dir.join("themes"))
        .unwrap_or_else(|| PathBuf::from("themes"))
}

/// Built-in themes followed by the user's; a user theme with a built-in id replaces it.
/// Returns the themes plus one "file: error" line per user file that failed to load.
pub fn load_all(user_dir: &Path) -> (Vec<ThemeDef>, Vec<String>) {
    let mut themes: Vec<ThemeDef> = BUILTIN
        .iter()
        .filter_map(|(id, source)| parse_with_base(id, source, None).ok())
        .collect();
    let mut errors = Vec::new();

    let mut files: Vec<PathBuf> = fs::read_dir(user_dir)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect())
        .unwrap_or_default();
    files.sort();

    for path in files {
        let Some(id) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else { continue };
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let result = fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|source| parse_with_base(&id, &source, path.parent()));
        match result {
            Ok(theme) => match themes.iter_mut().find(|t| t.id == theme.id) {
                Some(existing) => *existing = theme,
                None => themes.push(theme),
            },
            Err(e) => errors.push(format!("{}: {}", file_name, e)),
        }
    }
    (themes, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_formats() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba { r: 255, g: 255, b: 255, a: 255 });
        assert_eq!(parse_color("#3b82f6").unwrap(), Rgba { r: 0x3b, g: 0x82, b: 0xf6, a: 255 });
        assert_eq!(parse_color("#00000099").unwrap(), Rgba { r: 0, g: 0, b: 0, a: 0x99 });
        assert!(parse_color("3b82f6").is_err());
        assert!(parse_color("#3b82f").is_err());
        assert!(parse_color("#zzzzzz").is_err());
    }

    #[test]
    fn test_builtin_themes_are_complete() {
        for (id, source) in BUILTIN {
            let theme = parse_with_base(id, source, None).unwrap_or_else(|e| panic!("{}: {}", id, e));
            assert_eq!(theme.colors.len(), KEYS.len(), "{}", id);
        }
        // The base palettes must define every key themselves.
        for id in ["cutlery-dark", "cutlery-light"] {
            let source = BUILTIN.iter().find(|(n, _)| *n == id).unwrap().1;
            parse_theme(id, source, None, &BTreeMap::new()).unwrap_or_else(|e| panic!("{}: {}", id, e));
        }
    }

    #[test]
    fn test_missing_keys_inherit_from_base() {
        let theme = parse_with_base("t", "name = \"T\"\nbase = \"light\"\n[colors]\nsurface = \"#123456\"\naccent = \"#ff0000\"\n", None).unwrap();
        assert!(!theme.dark);
        assert_eq!(theme.color("surface"), parse_color("#123456").unwrap());
        assert_eq!(theme.color("window-bg"), base_palette(false)["window-bg"]);
        assert_eq!(theme.accent, Some(Rgba { r: 255, g: 0, b: 0, a: 255 }));
    }

    #[test]
    fn test_studio_section() {
        let source = "name = \"T\"\n[studio.remap]\n\"#353741\" = \"#44475a\"\n[studio.colors]\n\"MainBackground.Hover\" = \"#123\"\n[studio.syntax]\nKeyword = \"#ff79c6\"\n";
        let theme = parse_with_base("t", source, None).unwrap();
        assert_eq!(theme.studio.remap["353741"], parse_color("#44475a").unwrap());
        assert_eq!(theme.studio.colors["MainBackground.Hover"], parse_color("#112233").unwrap());
        assert_eq!(theme.studio.syntax["Keyword"], parse_color("#ff79c6").unwrap());
        assert!(parse_with_base("t", "name = \"T\"\n[studio.remap]\n\"#35374180\" = \"#000\"\n", None).is_err());
        assert!(parse_with_base("t", "name = \"T\"\n[studio]\nfoo = 1\n", None).is_err());
        assert_eq!(accent_by_name("rose"), parse_color("#f43f5e").unwrap());
        assert_eq!(accent_by_name("nope"), accent_by_name("blue"));
    }

    #[test]
    fn test_invalid_themes_are_rejected() {
        assert!(parse_with_base("t", "name = \"T\"\n[colors]\nsurfce = \"#000\"\n", None).is_err());
        assert!(parse_with_base("t", "name = \"T\"\n[colors]\nsurface = \"black\"\n", None).is_err());
        assert!(parse_with_base("t", "name = \"T\"\nbase = \"sepia\"\n", None).is_err());
        assert!(parse_with_base("t", "colors = {}\n", None).is_err());
    }

    #[test]
    fn test_user_themes_load_and_override() {
        let dir = std::env::temp_dir().join(format!("cutlery-themes-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mine.toml"), "name = \"Mine\"\n[colors]\ntext = \"#eeeeee\"\n").unwrap();
        fs::write(dir.join("oled.toml"), "name = \"My OLED\"\n").unwrap();
        fs::write(dir.join("broken.toml"), "name = \n").unwrap();
        fs::write(dir.join("notes.txt"), "ignored").unwrap();

        let (themes, errors) = load_all(&dir);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(themes.len(), BUILTIN.len() + 1);
        assert_eq!(themes.iter().find(|t| t.id == "oled").unwrap().name, "My OLED");
        assert!(themes.iter().any(|t| t.id == "mine"));
        assert_eq!(errors.len(), 1);
        assert!(errors[0].starts_with("broken.toml"));
    }
}

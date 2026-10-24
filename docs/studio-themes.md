# Roblox Studio themes: findings

Reference for applying Cutlery themes to Roblox Studio. Results come from a manual spike on
Studio `version-7aaa6838d36c4448` (0.744.0.7441077), Wine + DXVK, on 2026-10-24.

## How Studio loads its theme

- Studio is a Qt 5 application. Its UI colors come from two JSON files compiled into
  `RobloxStudioBeta.exe` as Qt resources:
  `:/Platform/Base/QtUI/themes/FoundationDarkTheme.json` and `...FoundationLightTheme.json`.
- These are the only two `:/Platform` strings in the exe. Replacing the 10 bytes `:/Platform`
  with `./Platform` in both (2 bytes change in total, file size unchanged) makes Qt read the
  files from `Platform/Base/QtUI/themes/` relative to the process working directory.
  Studio must therefore be started with the version folder as its working directory.
- The originals are stored qCompress-style: a 4-byte big-endian uncompressed size followed by
  a zlib stream. Scanning the exe for zlib streams that decompress to JSON with
  `"Name": "Dark"` / `"Name": "Light"` finds them (Dark ≈ 51.8 KB, Light ≈ 51.7 KB).
- Once patched, **both** paths point to disk: both files must always exist, or that theme
  fails to load.
- The Dark/Light choice is stored in the Wine registry:
  `HKCU\Software\Roblox\RobloxStudio\Themes`, `"CurrentTheme"="Dark"` (or `"Light"`).

## JSON layout

| Key | Content |
| --- | --- |
| `Name` | `"Dark"` / `"Light"` (keep as is). |
| `Presets` | Script Editor color presets `RobloxDefault`, `Extra1`, `Extra2`, `Custom`, 49 keys each: syntax colors plus `Background`, `ActiveLine`, `SelectionBackground`, `Menu*`… `GlobalSettings_13.xml` only stores which preset is selected (`Script Editor Color Preset`). |
| `Colors` | 155 `StudioStyleGuideColor` entries, each with `Default` and sometimes `Hover`/`Selected`/`Disabled`/`Pressed`. |
| `Paths` | Icon paths (`:/images/...`); leave untouched. |
| Widget sections | `Ribbon`, `CommonStyle`, `Menu`, `Scrollbar`, `Combobox`, `Checkbox`, `ToastNotification`, `StartPage`, … |
| Foundation tokens | `Background`, `Text`, `Primary`, `Secondary`, `Action`, `Semantic`, `Success`, `Warning`, `Error`, … |

Colors are `#RRGGBB` or Qt's `#AARRGGBB` (alpha **first**; Cutlery uses `#RRGGBBAA`).

The palette is small: Dark uses 70 distinct colors, dominated by
`#353741 #202227 #BCBEC8 #191A1F #6A6F81 #494D5A #528BFF #FFFFFF #272930 #335FFF`
(Light: 76, dominated by `#FFFFFF #BCBEC8 #000000 #6A6F81 #E6E7EA #EEEFF1 #D5D7DD #EBF1FF`).
Remapping these by value themes everything the JSON reaches.

## What the JSON themes

Tested by remapping `#191A1F`, `#202227`, `#272930`, `#353741`, `#494D5A`, `#121215` to loud
colors and the `Presets` `Background`/`ActiveLine`/syntax colors.

| Area | Themed? |
| --- | --- |
| Menu bar | Yes (`#272930`) |
| Dock title bars, document tabs, borders, splitters, scrollbars | Yes |
| Script Editor: background, gutter, active line, syntax | Yes (`Presets`) |
| Command bar | Yes (editor `Presets` background) |
| Properties panel (Qt widget, incl. during Play) | Yes |
| Output panel | Yes |
| Ribbon (toolbar and tabs) | **No** |
| Explorer | **No** |
| Toolbox | **No** |
| Window title bar | No (drawn by the window manager) |

## Why the ribbon, Explorer and Toolbox are not themed

They are built-in Luau plugins (`BuiltInStandalonePlugins/Optimized_Embedded_Signature/Ribbon.rbxm`,
`ExplorerPlugin.rbxm`, `BuiltInPlugins/.../Toolbox.rbxm`). Their colors come from
DeveloperFramework `FoundationDark`/`FoundationLight` tokens compiled into the plugin bytecode,
not from the Qt JSON (remapping every JSON color left them unchanged). Each `.rbxm` carries a
`SIGN` chunk, so editing them would break the signature. Out of scope.

## Patched copy vs. in-place patch

A patched copy next to the original (`RobloxStudioCutlery.exe`) works: login, opening a place,
Play (client + server) and the Properties/Output panels behaved normally. Keeping the original
untouched means Studio launched outside Cutlery is unaffected. Not yet tested with the copy:
`roblox-studio://` links, Team Create and the MCP bridge.

## Spike leftovers

The spike left `RobloxStudioCutlery.exe` and `Platform/Base/QtUI/themes/` (with marker colors)
in the version folder. The original `RobloxStudioBeta.exe` is untouched and keeps using its
built-in theme.

# Changelog

The Control Center home page shows this file from the installed release, so edits here appear without recompiling.

## v1.5.0

- **Themes**: the Control Center ships Cutlery Dark, Cutlery Light, Midnight and OLED Black, with a live preview in Graphics → Appearance. Add your own as TOML files in `~/.config/cutlery/themes/`; a theme can fix the accent color and replace the Home artwork.
- **Themes in Roblox Studio (experimental)**: the selected theme can also recolor Studio's panels, menus, tabs, Properties, Output and Script Editor. The ribbon, Explorer and Toolbox keep Studio's colors. The original Studio executable is never modified, and the Doctor page shows whether your Studio build is supported.
- **Window resizing fixed**: dragging the window edges no longer leaves the mouse stuck resizing.

## v1.4.0

- **Working Update button**: checks GitHub Releases for a newer Cutlery, asks before installing (verified with SHA-256) and restarts into the new version. Roblox Studio is then updated only when Roblox has published a newer build.
- **`cutlery update`**: the same update flow from the terminal.
- **Installed versions everywhere**: the Cutlery, Roblox Studio and DXVK versions shown in the UI are read from the installation instead of being hardcoded.
- **Scrollable changelog**: the home page lists every release from the installed CHANGELOG.md, with the installed version highlighted.

## v1.3.0

- **Rebuilt Control Center**: a single interface based on the Figma design, split into reusable Slint components and pages.
- **Resizable window**: frameless window with edge resizing, working maximize (also by double-clicking the title bar) and a 960×600 minimum size.
- **No more freezes**: Studio install/update and diagnostics run in the background, with progress shown in the new status bar.
- **Reliable unapplied-changes tracking**: Apply/Revert compare against the saved config, including custom FastFlags.
- **Doctor page**: environment info, health checks and Studio install/update in one place.

## v1.2.1

- Sanitized `roblox-studio://` URI arguments, removed hardcoded paths and stopped tracking prebuilt shared libraries.

## v1.2.0

- **Visual remake**: Figma-based interface with vertical icon navigation, page headers and accent color themes.
- **Version detection**: reads the installed Roblox Studio and Cutlery versions.
- **Frameless hero launch**: launch Studio from the home page, with an unapplied-changes guard.
- **Universal MCP Bridge & DXVK v3.1.1**: pre-init shield, async shader pipelines and NTSync synchronization.

## v1.1.0

- SHA-256 verification of downloaded DXVK releases.

## v1.0.0

- First release: rootless docking fix, DXVK setup, Studio installer and Universal MCP Bridge.

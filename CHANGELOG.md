# Changelog

The section matching the running version is shown on the Control Center home page.

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

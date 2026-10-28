# Cutlery Assets

This directory contains the logo and icon assets for the Cutlery project.

### Recommended Files & Formats:
- **`cutlery.png`**: High-resolution 512x512 RGBA image with transparent background (used by standard Linux application launchers, GNOME, and KDE).
- **`icons/`**:
  - `cutlery-16x16.png`
  - `cutlery-32x32.png`
  - `cutlery-48x48.png`
  - `cutlery-64x64.png`
  - `cutlery-128x128.png`
  - `cutlery-256x256.png`

The PNG icon variants are resized from the 512x512 source with their square aspect ratio and transparent background preserved. Running `cutlery register-desktop` automatically installs them into standard FreeDesktop paths (`~/.local/share/icons/hicolor/...`) and updates the system icon cache.


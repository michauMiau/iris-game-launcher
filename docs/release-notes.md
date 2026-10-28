# Iris Game Launcher v0.1.0

Iris Game Launcher is a desktop library manager for PC VR games you already own.
It is not affiliated with any storefront, and it contains no game content —
downloads happen through your own account login, and nothing bypasses purchase
checks.

## This version
- Library shows only games that have a PC build; headset-only entries are no
  longer listed (installing them used to fail with a refusal from the CDN).
- Per-game page: artwork, Install / Play, ⋯ menu with Check updates,
  Show versions, Verify files, Browse files, Uninstall, MelonLoader installer,
  Proton version choice, Run another exe in the prefix, Wine settings (Linux).
- Downloads show a progress strip with file and byte counts and a Cancel
  button; Proton GE and MelonLoader installs show the same strip.
- Login status is verified against the store before claiming signed in.
- Themes: dark, light, solarized. Languages: English, Polski (modular table —
  more languages are just new tables).
- Crashes from graphics drivers now leave a "gui starting" line in
  launcher.log so the point of failure is visible; --safe forces software
  rendering, --no-covers skips cover textures (for low-VRAM machines).

## Files in this release
- IrisLauncher-x86_64.AppImage (Linux)
- IrisLauncher-win-x86_64.exe (Windows)

## Also in v0.1.0
- Import from disk asks which library game a folder is, auto-matching store
  manifests and folder names, with a plain-folder escape; matched games behave
  exactly like store installs.
- Folder and exe pickers use the desktop's own dialog (zenity, kdialog, yad,
  PowerShell), falling back to the built-in chooser only when none is installed.
- Achievement unlocks made by games are recorded into the shim trace log in the
  game folder, groundwork for the achievements section.
- Windows: the runtime swap moves the headset plugin to the SteamVR translation
  layer (one UAC prompt, originals backed up), restore available in settings.
- The AppImage carries repository and update metadata, so AppImage update
  tools can pull new versions from GitHub releases directly.
- License: GPL-3.0; the vendored Revive translation layer keeps its MIT
  attribution.

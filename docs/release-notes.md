# Iris Game Launcher — release notes (draft, not pushed)

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

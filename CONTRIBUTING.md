# Contributing to Iris Game Launcher

The launcher is Rust (`launcher/`), plus two vendored shim crates (`shim/`, `ovrplugin-shim/`)
that build Windows DLLs with the mingw-w64 target. Both shims are single files: the launcher
embeds them via `include_bytes!` and copies them into game directories at launch time.

## Tech stack

- **Runtime**: Rust, `x86_64` for the launcher itself; shim DLLs target `x86_64-pc-windows-gnu`.
- **UI**: iced (native Rust GUI, no webviews).
- **Packaging**: AppImage (Linux), single exe (Windows).
- **Tests**: Rust unit tests in `launcher/src/tests/` (launch, vr, stop, assets, settings, fetcher, auth, i18n), Wine-translated pass through the exact same binaries CI runs.

## Repository layout

```
launcher/          the app (iced UI, fetch machinery, auth layer)
shim/              LibOVRPlatform64_1.dll replacement (build.rs generates the export stubs)
ovrplugin-shim/    Data/Plugins/OVRPlugin.dll replacement (ovrp_* ABI surface)
revive-layer/      vendored ReviveNext translation layer (LibOVRRT replacement, OpenXR-backed)
assets/            vendored DLLs embedded by the launcher (platform-shim.dll, ovrplugin-shim.dll,
                   LibReviveXR64.dll, LibRevive64.dll), AppIcon, screenshot
AGENTS.md          agent-facing architecture + build instructions (same short text is injected
                   into every agent session through the plugin section)
```

## Building

```bash
# launcher (Linux or Windows target)
cd launcher && cargo build --release
cd launcher && cargo build --release --target x86_64-pc-windows-gnu

# platform shim
cd shim && cargo build --release --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/platform_shim.dll ../assets/platform-shim.dll

# OVRPlugin shim
cd ovrplugin-shim && cargo build --release --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/ovrplugin_shim.dll ../assets/ovrplugin-shim.dll
```

The mingw target needs the cross toolchain on Linux (`sudo apt-get install -y mingw-w64`) and
`rustup target add x86_64-pc-windows-gnu`.

## The gate (run everything, every time)

1. `cargo fmt` in `launcher/`
2. `cargo test` (Linux host) and the same suite compiled for `x86_64-pc-windows-gnu`, executed under Wine — the Linux pass and the Wine pass are separate by design (they historically diverged; keep both green)
3. `cargo clippy --quiet` for both targets must print **zero** warnings
4. Release builds for both targets
5. `tests/assets.rs`: every vendored DLL in `assets/` must have a nonzero size

CI runs the same commands on `main`, on Linux and Windows runners. Anything that fails there fails the merge.

## Releases

1. Gate green.
2. `bash pkg-iris.sh` (AppImage + zip in `dist/`).
3. `git commit`, `git push origin main`, wait for CI green.
4. `git tag vX.Y.Z && git push origin vX.Y.Z`.
5. `gh release create vX.Y.Z --target main --title "Iris Game Launcher vX.Y.Z" --notes "<text>" dist/IrisLauncher-x86_64.AppImage dist/IrisLauncher-x86_64.AppImage.zsync dist/IrisLauncher-win.zip`.

## Working on the shims

Both shims follow the same discipline:

- **Export surface first**: hook the C# `DllImport` names from `OVRPlugin.cs` (export tables were
  extracted from genuine DLLs with pefile; see `pe_exports.py` in each crate).
- **Unimplemented functions are inert**: return benign zeros and never act on pointers. A wrong
  arity is an ABI footgun on x64 Windows; when unsure, declare the argument and ignore it.
- **Logging never aborts the game**: trace writes must never panic across the FFI boundary; an
  unwritable log path is silently off.
- **Re-vendor after every shim change**: rebuild, copy to `assets/*.dll`, run `cargo test` in
  `launcher/` — the embedded-bytes test catches drift.

## Conventions

- Comments explain *why*, not what. No narrating obvious steps.
- Log lines in the launcher describe what actually happened (manifest pointed at, vrserver started
  or not, swap landed or declined), never promises about live VR sessions.
- Per-game mutable state lives beside the game (game dir or `<data_dir>/prefixes/<game>`), never
  in a single shared folder.
- Tests are grouped by module under `launcher/src/tests/`, each `use crate::*`. No `#[test]` inside
  module files. Anything a test needs becomes `pub(crate)`.

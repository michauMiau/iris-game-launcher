# Contributing guide for Iris Game Launcher

Thanks for deciding to contribute to this project!
We welcome help to this project.
Generally most contributions will be accepted after careful review.

The launcher is Rust (`launcher/`), plus two vendored shim crates (`shim/`, `ovrplugin-shim/`)
that build Windows DLLs with the mingw-w64 target. Both shims are single files: the launcher
embeds them via `include_bytes!` and copies them into game directories at launch time.

Every change lands the same way:

1. `cargo fmt --check` in `launcher/` — no diff allowed.
2. `cargo test` (Linux host) and the same suite compiled for `x86_64-pc-windows-gnu`, executed under Wine — the Linux pass and the Wine pass are separate by design (they historically diverged; keep both green).
3. `cargo clippy --quiet` for both targets must print **zero** warnings.
4. Release builds for both targets.
5. If you touch a shim crate, rebuild its DLL and copy it into `assets/`; the embedded-bytes test in `tests/assets.rs` guards that copy from drifting. CI rebuilds the shims on every `main` push, so a forgotten copy is caught there too.

Then: `git commit`, `git push origin main`, CI must be green.

## Tech stack

- **Runtime**: Rust, `x86_64` for the launcher itself; shim DLLs target `x86_64-pc-windows-gnu`.
- **UI**: iced (native Rust GUI, no webviews).
- **Packaging**: AppImage (Linux), one exe (Windows). `bash pkg-iris.sh` builds both in `dist/`.
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

## Contributing a language

The localizations for the project are stored in `launcher/src/i18n.rs`.
The file is pretty self-explanatory; adding new languages is pretty easy.
New keys go into both the EN and PL tables, and the i18n completeness test catches a missing row.
Keep translations short and in the existing tone — user-facing strings only; internal log lines stay in English.

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
- Tests are grouped by module under `launcher/src/tests/`, each `use crate::*`. No `#[test]` inside module files. Anything a test needs becomes `pub(crate)`.

## Updating the compatibility list

`docs/game-compatibility.md` tracks per-title status. A row moves only against a live run on
the user's box — title, VR path used, exact symptom when it fails, and which log proved it.
Never promote a row to "verified" on a stub or a local-only check. The OVRPlugin rows stay
**experimental** until a captured VR session exists.

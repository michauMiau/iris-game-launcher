# Iris Game Launcher v0.54.0

Iris Game Launcher is a desktop library manager for PC VR games you already own.
It is not affiliated with any storefront, and it contains no game content.

## This release (v0.54.0)

### OVRPlugin shim (0.1.35) — collective compatibility fixes
Targeted at Oculus-integrated Unity titles (experimental path, e.g. Boneworks via OVRPlugin/Revive/SteamVR under Proton):

- **Initialization:** report `ovrp_GetInitialized` optimistically once runtime bring-up is requested so Unity can proceed to VR/render queries while LibReviveXR session creation runs asynchronously (instead of stalling on the desktop path on init races/failures).
- **Time/pointers:** correct `ovrp_GetPredictedDisplayTime` to return a valid `f64` predicted display time (plus related time helpers), and return a stable non-null native SDK pointer.
- **Display/render descriptors:** always report sane eye/FOV texture sizes and improved HMD descriptor data.
- **Diagnostics and ABI coverage:** add tracing to previously untraced exports and add traced stubs/helpers for common render/session/presence paths (swapchain create/get/commit for DX/GL/Vk, frame submit, node pose/display-time/presence and session-status queries) so a single run can identify remaining interop gaps.
- Bundled updated `assets/ovrplugin-shim.dll` and bumped launcher to 0.54.0.

**Status:** launch/init and plugin-validation blockers addressed incrementally, but VR compositor/render submission interop remains experimental and may still require follow-up based on a fresh HMD trace.

### Since v0.53
- **v0.53:** UI fixes (single channel selector, clearer achievements) and stop guessing `VR_RUNTIME_DIR=/tmp`.
- **v0.52:** Restore/fix OVRPlugin shim (0.1.34) with XR-only auto bring-up, exe-dir DLL loading and diagnostics; avoid Revive64/OpenVR `ovr_Initialize` crash.
- **v0.51–v0.49:** Prefer OpenXR-backed Revive (`LibReviveXR64.dll`) for OVRPlugin runtime bring-up, place both Revive backends where needed, and improve runtime DLL loading diagnostics.

## Files in this release
- `IrisLauncher-x86_64.AppImage` (Linux)
- `IrisLauncher-win-x86_64.exe` (Windows)

## Notes
- License: GPL-3.0; the vendored Revive translation layer keeps its MIT attribution.
- Game compatibility is tracked in `docs/game-compatibility.md`. Not affiliated with any storefront; no game content included.

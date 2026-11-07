# Iris Launcher Agents file

Iris launcher is a replacement for the headset vendor desktop app.
It is meant to launch, download and manage PCVR games owned in the vendor store library.
It is NOT meant to stream games, configure VR on Linux or anything like that.
It HAS to be lightweight, and run both on Windows and Linux.

## The Architecture

Both platforms (Windows, Linux) share the same components:

- Store client: fetches the owned game list through the account session, downloads PCBinary builds, re-verifies them, keeps the library grid and the per-game page (Install / Play / Stop, ⋯ menu, import folder).
- Fetch machinery (`launcher/src/fetcher.rs`): content-length verified downloads, resume with Range, zip unpack that refuses truncated archives, GitHub release fetchers for Proton builds and MelonLoader assets.
- Auth/session layer (`launcher/src/auth.rs`): token handling (masked in UI, reveal / write-to-disk / forget), `launcher.log`, truncated at startup.
- UI core: themes, EN/PL i18n tables (`launcher/src/i18n.rs`), Proton version popup, launch-option radios, Running state with Stop.
- The Shim, described below.

### The Shim (what makes this special and allows running games without any vendor software)

Headset PC binaries built against the vendor runtime talk to the platform API at boot: Unity calls the init wrappers, then the game sends an entitlement request through `LibOVRPlatform64_1.dll` and polls `ovr_PopMessage` while it waits. Without vendor software installed that DLL does not exist, the game throws `DllNotFoundException: LibOVRPlatform64_1`, logs `Entitlement Failed`, and dies in its scene bootstrapper (this was the exact Bonelab crash).

The shim is a rebuilt `LibOVRPlatform64_1.dll` (`shim/`, export table copied from the genuine runtime, 1209 exports). It:

- answers Unity's init wrappers as initialized,
- queues the entitlement request and pops back a non-error message carrying the game's App Id, so viewer-entitlement checks pass with no network and no vendor service,
- returns empty strings and success codes from the probes games make (`Error_GetCode`, `Error_GetHttpCode`, `Message_GetRequestID`, the string getters),
- keeps the PopMessage/FreeMessage poll loop cheap instead of answering thousands of null polls with logging.

After any change in `shim/` rebuild it and copy the DLL to `assets/platform-shim.dll`; the embedded-bytes test in tests/assets.rs guards that copy from drifting. The launcher copies the shim beside the game exe under that exact name on every launch, on both platforms. The `Shim trace log` radio opts in to a diagnostic timeline: `<ms-since-load> <export-name>` lines with popped message type/id/error, written to `iris-shim-trace.log` in the game folder.

### The OVRPlugin shim (second interception point, for titles whose plugin never loads a runtime DLL)

The platform shim above is not the whole story. Older Unity titles ship their own `Data/Plugins/OVRPlugin.dll` (the Oculus Unity native plugin), which statically links the vendor runtime and initializes against its service — a complete loaddll trace of Boneworks shows zero load attempts of `LibOVRRT64_1.dll` at all, so no beside-exe DLL placement can ever intercept it; Unity reports `SDK v0.0.0` and `Security error. Oculus plugin is invalid!`.

`ovrplugin-shim/` (Rust cdylib, mingw Windows target, mirrors `shim/`'s build discipline) replaces that file at launch, export-for-export verified identical to the real plugin's 412 names. It answers the plugin identity questions truthfully (`ovrp_GetVersion` and `ovrp_GetNativeSDKVersion` report `1.43.0`; `ovrp_GetInitialized` reflects the real session state), and its initialize path loads the runtime we already place (`LibOVRRT64_1.dll` beside the exe) and creates the session through `ovr_Initialize`/`ovr_Create`, keeping it alive for the process lifetime. Remaining exports are inert stubs returning benign values; only the init path is wired. The launcher keeps the original beside the file as `OVRPlugin.dll.orig`; deleting our copy and renaming the `.orig` back restores the exact original state. The swap runs behind the same SteamVR-patch gates as the other placements; it is not needed (and never runs) on Windows, where the real plugin loads its runtime normally.

Both shims share one trace switch: the `Shim trace log` radio sets `OVR_SHIM_LOG` for the launch, and either shim that finds the variable appends `<ms-since-load> <call-name>` lines to that path when it is set. The platform shim logs every call (plus its achievement ids and null-poll heartbeat); the plugin shim logs its initialize hops — runtime not found, missing `ovr_Initialize`/`ovr_Create`, init failure, session created — and the first three begin/end frames, then one line every 600 frames, so a traced live run shows exactly which hop the session died at with no per-frame spam. An unwritable log path stays silently off in both shims (a panic across the FFI boundary is UB; the game keeps running either way).

After any change in `ovrplugin-shim/` rebuild it and copy the DLL to `assets/ovrplugin-shim.dll`; CI does the same on every push, and the pefile export check keeps the cover table at exactly the real plugin's 412 names.

The shim also carries achievement ids into the trace: `ovr_Achievements_Unlock` logs `<ms> ovr_Achievements_Unlock <achievement-id>` (verified under wine against a probe exe). Bonelab's live successful-run trace shows three Unlock calls, so the planned achievements section on the game page will build its list from these trace lines.

The shim covers the Platform API only. The VR/compositor side the headset plugin uses (LibOVRRT) is answered by the translation layer, which is platform-specific.

Unity-era plugins on Wine import the libovr API from `*_Data/Plugins/[x86_64|x64]/OVRPlugin.dll` — that file IS the interceptable name (the plugin interface is the `ovr_*` set, confirmed by dumping the exports of Boneworks' shipped plugin), so `librevive_rt_step` places the translation DLL there too, originals backed up to `data/ovr-backup`. Beside-exe LibOVRRT64_1.dll covers games that import by that name.

## Where the difference shows

### Linux

The Linux version of the launcher also has a built-in proton downloader and a proton launcher, to launch games using wine. The shim stays the same, ReviveNext is also modified to fit better: only its translation layer is vendored (`assets/LibReviveXR64.dll`, an OpenXR-backed LibOVRRT replacement exporting `ovr_*`), nothing of the ReviveNext app itself.

Linux VR wiring is automatic, the launcher never asks the user to set anything up. SteamVR Linux is assumed present (Steam Link VR games already work through Steam). On launch the launcher:

- detects the SteamVR install (`steamapps/common/SteamVR`, recognized by `bin/linux64/vrclient.so`),
- persists `PROTON_VR_RUNTIME` on the game so Proton's vrclient bridge loads that runtime,
- exports `XR_RUNTIME_JSON` pointing at SteamVR's shipped `steamxr_linux64.json`, because Wine's OpenXR loader searches the prefix's XDG dirs and never sees the host manifest (and SteamVR's own "set as OpenXR runtime" button is broken, Valve issue #840),
- mirrors the host OpenVR path registry (`~/.config/openvr`) into the game prefix, kept separate from OpenXR wiring,
- detects a live `vrserver` and logs a netns diagnostic; it never starts vrserver itself (SteamVR ships its own .desktop handlers),
- when the vrserver is live but the chosen Proton build cannot open XR outside Steam, logs the reason: Valve's wineopenxr only initializes through the `HKCU\Software\Wine\VR` registry key that `steam_helper` writes for Steam-launched games (Valve issues #7382 and #8256), so outside Steam every xr call returns `XR_ERROR_RUNTIME_UNAVAILABLE` regardless of env vars, netns or abstract sockets; the fix is GE-Proton 10-33 or newer, which initializes that registry inside wineopenxr itself, downloadable through the Proton versions popup (`proton_vr_capable` reads the build name).

Games launch through `proton waitforexitandrun` with `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH` and a numeric `SteamAppId`, the same call shape Steam uses, so protonfixes do not treat the run as a unit test. One prefix per game (`prefixes/<game>`), Stop kills the whole process group.

### Windows

Windows launches the game exe directly, no wine. The launcher places the same platform shim beside the exe and the translation DLL as `LibOVRRT64_1.dll` beside it too. That alone is not enough on Windows: the Unity headset plugin resolves LibOVR from the vendor runtime install folder (its log names `...Support/oculus-runtime/LibOVRRTImpl64_1.dll`), so beside-exe copies never redirect it. The launcher therefore swaps the runtime folder itself, `LibOVRRT64_1.dll` and `LibOVRRTImpl64_1.dll`, with the translation layer through one elevated PowerShell script. UAC is asked once per machine, the originals are backed up to `iris-runtime-backup`, and Settings has a Restore button to put them back. That swap is the headset link slot takeover: every plugin game on the machine reaches SteamVR instead of the vendor service. The launcher also locates Steam (registry `HKLM\SOFTWARE\Valve\Steam`, common folders as fallback), points `XR_RUNTIME_JSON` at SteamVR's manifest, and expects SteamVR to be already running: the launcher never launches vrserver itself, SteamVR owns its desktop handlers. The ReviveNext installer stays available in Settings as the alternative layer (NSIS setup, needs UAC, run via `Start-Process -Verb RunAs`).

## Rules for agents working on this repo



- Don't save Commit SHAs and package hashes into memory.
- Build from `launcher/` only. Windows artifacts are built for `x86_64-pc-windows-gnu`; packages are single files (AppImage on Linux, one exe on Windows), no extra containers or side binaries.
- Keep launches lightweight and truthful: log lines say what actually happened (manifest pointed at, vrserver started or not, swap landed or declined), never promises about live VR sessions.
- Clean code is preferrable over a wall of comments. Comment what the code cannot show (why, traps, measured facts); do not narrate obvious steps.
- Tests live outside module files: grouped modules in `launcher/src/tests/` (launch, vr, stop, assets, settings, fetcher, auth, i18n), each `use crate::*`. No module file carries `#[test]`; whatever a test needs becomes `pub(crate)`.
- First run (no config.json yet) matches the desktop: language from locale vars or Windows Locale registry, theme from GTK_THEME / GNOME color-scheme / KDE kdeglobals / Windows AppsUseLightTheme. Unknown system hints fall back to shipped defaults; a malformed existing config also falls back to defaults instead of re-detecting.

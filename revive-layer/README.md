# Internal fork: ReviveNext translation layer only

Vendored from https://github.com/Konstruukt-Labs/ReviveNext (MIT, `LICENSE`
copied from the vendored `revive/` subtree) as of commit `master` fetched
2026-10-26. The fork keeps ONLY the translation layer, no GUI app:

- `revive/Revive` - `LibRevive64.dll`, the LibOVR/Oculus API replacement that
  translates Oculus calls into OpenVR (SteamVR).
- `revive/ReviveXR` - `LibReviveXR64.dll`, an OpenXR runtime layer that talks
  to SteamVR through the OpenXR loader.
- `revive/ReviveInjector` - `ReviveInjector.exe`, hooks game processes so the
  translation DLLs land inside them.

Deliberately excluded from the upstream repo: `src/` (the Win32 GUI app, art
matcher, SteamGridDB sync, title overrides), `installer/ReviveNext.nsi`, the
root app CMakeLists, Art assets. Iris Game Launcher supplies game discovery,
install UI and the library, so the app GUI is not needed here.

## Building (Windows, MSBuild route like upstream CI)

Init the submodules the vendored tree needs headers from:

    git submodule add https://github.com/ValveSoftware/openvr.git revive/Externals/openvr
    git submodule add https://github.com/KhronosGroup/Vulkan-Headers.git revive/Externals/Vulkan
    git submodule add https://github.com/zeux/microprofile.git revive/Externals/microprofile
    git submodule add https://github.com/nlohmann/json.git 3rdparty/json

Then the same one-time deps as upstream BUILDING.md:

    vcpkg integrate install
    vcpkg install --triplet x64-windows-static detours openxr-loader

Oculus SDK LibOVR (Meta's own license, not redistributable here): download
https://securecdn.oculus.com/binaries/download/?id=4377593722298679 and place
the `LibOVR/` folder at `revive/Externals/LibOVR/`.

Build: `msbuild revive/Revive.sln /p:Configuration=Release` produces the three
outputs above.

## How Iris uses it

Settings already scan Program Files, LOCALAPPDATA and the launcher tools dir
for a Revive layer (`LibOVRRT*.dll`, `OVRServer_x64.exe`). At launch on
Windows the launcher copies the configured LibOVRRT dll beside the game exe.
If `ReviveInjector.exe` is present in the launcher tools dir it is started
before the game so the hook lands in the game process. The fork's outputs
belong in `~/.local/share/iris-launcher/tools/` (Windows: `%LOCALAPPDATA%\iris-launcher\tools\`)
or in an install dir the Settings fields point at. No SteamVR app registration
is done by Iris - the game runs directly, the layer translates the calls.

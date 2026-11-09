# Game compatibility

## Known working (verified in-headset)

| Title | State |
|---|---|
| **Bonelab** (Unity OpenXR) | Works, requires HandsStuck mod, only the Steam version got the patch, Fusion broken, use FusionHelper or a different networking layer |

## Experimental

| Title | Path | State |
|---|---|---|
| **Boneworks** (Unity Oculus integration, SDK 1.43, `Data/Plugins/OVRPlugin.dll`) | OVRPlugin shim → LibRevive64 (OpenVR) → SteamVR | **Does not hand off a session.** Unity loads the shim and reads identity strings (`SDK v1.43.0` prints), then throws `Security error. Oculus plugin is invalid!` and crashes in managed code (GameAssembly null read) before any `ovrp_Initialize` call — verified across shim builds v0.1.13–v0.1.21 via `iris-ovrplugin-trace.log` (zero plugin-side records beyond identity, platform records only). The hooks that would fix it — exact 1 .43-era native ABI (`ovrpFunctionType` registration, struct layouts) — are a guess until the real `OVR_Plugin.h` is available; stubs inferred from a newer C# binding accidentally worked farther than stubs guessed from an older one, so no ABI guess is treated as trustworthy. Game renders flat / persists on screen when left alone (NaN-state render). |

# Game compatibility

## Known working (verified)

| Title | Notes |
|---|---|
| **Bonelab** (Unity OpenXR) | Works, requires HandsStuck mod, only the Steam version got the patch, Fusion broken, use FusionHelper or a different networking layer |

## Experimental

| Title | Path | State |
|---|---|---|
| **Boneworks** (Unity Oculus integration, SDK 1.43, `Data/Plugins/OVRPlugin.dll`) | OVRPlugin shim → LibRevive64 (OpenVR) → SteamVR under Proton | Launches to window; VR session/rendering not working yet. Requires SteamVR + connected HMD. Runtime bring-up and `ovrp_*` pose/swapchain/submit interop still in progress (v0.47+: bring-up on by default, `IRIS_OVR_DISABLE_RUNTIME=1` to opt out). |

## Untested / blocked

- Other Oculus-integrated Unity titles: may need the same OVRPlugin/OVR path; untested until a live run confirms session and frames.
- Non-Unity Oculus titles: not covered by the current Unity-oriented shim path.
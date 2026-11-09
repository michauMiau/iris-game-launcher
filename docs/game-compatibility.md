# Game compatibility

Per-title status of what the launcher gets working, with the mechanism that does it.
Status date: 2026-11-07 (v0.1.13). Statuses are updated only against live runs on the
user's box (RTX 4070 + SteamVR Linux). Everything running through the OVRPlugin shim is
still **experimental** — see the OVRPlugin column.

Platform for all rows: **Linux, Proton (GE-Proton ≥ 10-33 works for OpenXR outside Steam;
SteamVR Linux present)**. Windows keeps the vendor runtime installed and trades it for the
translation layer through a one-time elevated swap; it is the primary target everywhere
except where noted.

## Known working (verified in-headset)

| Title | Path | State |
|---|---|---|
| **Bonelab** (Unity OpenXR, i.e. Microsoft OpenXR Plugin for Unity) | Unity OpenXR → wineopenxr → SteamVR; platform shim answers init + entitlement | Functional (controllers live, haptics, SteamVR overlay) |
| **Xalia** (Unity OpenXR) | Unity OpenXR → wineopenxr → SteamVR | Functional (same stack as Bonelab) |

## Experimental

| Title | Path | State |
|---|---|---|
| **Boneworks** (Unity Oculus integration, SDK 1.43, `Data/Plugins/OVRPlugin.dll`) | OVRPlugin shim → LibRevive64 (OpenVR) → SteamVR | **Does not hand off a session.** Unity loads the shim and reads identity strings (`SDK v1.43.0` prints), then throws `Security error. Oculus plugin is invalid!` and crashes in managed code (GameAssembly null read) before any `ovrp_Initialize` call — verified across shim builds v0.1.13–v0.1.21 via `iris-ovrplugin-trace.log` (zero plugin-side records beyond identity, platform records only). The hooks that would fix it — exact 1 .43-era native ABI (`ovrpFunctionType` registration, struct layouts) — are a guess until the real `OVR_Plugin.h` is available; stubs inferred from a newer C# binding accidentally worked farther than stubs guessed from an older one, so no ABI guess is treated as trustworthy. Game renders flat / persists on screen when left alone (NaN-state render). |

## Other titles (unsupported or untested by Iris)

| Title | Mechanism Iris would reach for | State |
|---|---|---|
| **Beat Saber** (Unity OVRPlugin path) | OVRPlugin shim path, same as Boneworks | Untested on the launcher |
| **Half-Life: Alyx** | Native OpenVR | Untested on the launcher |
| **The Walking Dead: Saints & Sinners** | Unity OVRPlugin path | Untested on the launcher |

## Status legend

- **Functional** — boots, session created, head + controllers tracked in-headset on a recent live run.
- **Experimental** — infrastructure in place (DLLs landed, plugins accepted), but no captured successful session.
- **Untested** — expected to work via a listed path; no live run yet.
- **Blocked** — the stack has a known wall; see AGENTS.md ("Valve issues #7382 and #8256" for the
  wineopenxr/stub-vrserver wall, and the shim notes for the OVRPlugin wall).

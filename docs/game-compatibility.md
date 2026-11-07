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
| **Boneworks** (Unity Oculus integration, SDK 1.43, `Data/Plugins/OVRPlugin.dll`) | OVRPlugin shim → LibRevive64 (OpenVR) → SteamVR | Shim in place: Unity reports `SDK v1.43.0`, plugin loads and accepts identity queries. No captured VR session yet; plugin swap in place since v0.1.12, traced init since v0.1.13. Needs the headset + SteamVR live to verify. |

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

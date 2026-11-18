# Game compatibility

Games that use OpenXR (like Bonelab) talk to SteamVR directly and work out of the box.
Games that use the older Oculus plugin (like Boneworks) need the Oculus runtime, which
the launcher replaces with its own translator — those are the tricky ones.

## Known working (verified)

| Title | Notes |
|---|---|
| **Bonelab** (Unity OpenXR) | Works, requires HandsStuck mod, only the Steam version got the patch, Fusion broken, use FusionHelper or a different networking layer |

## Experimental

| Title | State |
|---|---|
| **Boneworks** | Launches, but the headset view does not appear yet. Linux test pending; on Windows the game refuses to start (it checks the plugin's signature). |

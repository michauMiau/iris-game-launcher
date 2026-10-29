<div align="center">
  <img src="assets/iris-logo.png" alt="Iris Game Launcher logo" width="170">
  <h1>Iris Game Launcher</h1>
  <h5>20 MiB file, 20 MiB VRAM, 90 MiB RAM, for the whole app.</h5>
</div>

<div align="center">
A lightweight replacement for the Meta Quest Link (formerly Oculus)
Play all your bought games on Linux and Windows.

Use your system resources for your games, not the launcher.
</div>

<div align="center">
  <img src="assets/screenshot.png" alt="Iris Game Launcher Screenshot with library games" width="760">
</div>

## Is this project SLOP?

While this project has been *vibecoded* (and I'm not proud of it), care has been taken to ensure this project is free of bugs, problems and regressions. 
This App has been rigorously tested by me, and I'm confident of its quality.

This project has been made entirely using local, open-weight models run entirely on my own lackluster hardware (RTX 4070, 32 GB DDR4, not a 4x DGX Spark cluster). 
No Claude or ChatGPT here.


## Install

- Linux: grab the AppImage from the [releases](https://github.com/michauMiau/iris-game-launcher/releases) page, `chmod +x IrisLauncher-x86_64.AppImage`, run it. Needs FUSE (or `--appimage-extract-and-run` if you don't have FUSE).
- Windows: grab `IrisLauncher-win-x86_64.exe` from the [releases](https://github.com/michauMiau/iris-game-launcher/releases) page and run it.

## Game Compatibility

| Game    | Status |
| :--------: | ------- |
| BONELAB  | ✅ Leaderboards and achievements not implemented, Fusion broken |
| BONEWORKS | ❌ Will fix in upcoming release |


## License

GPL-3.0. The OpenXR translation layer (Revive derivative) is MIT.

# Command line

Every option is optional; running with none opens the window.

| Option | What it does |
|---|---|
| `--help` | Print this list and quit. |
| `--launch <game name>` | Launch the named game straight away (used by the per-game desktop entries). |
| `rescan` | Re-scan the library folders, print what was found, quit. |
| `--download <meta app id> [folder]` | Download a store game by id into the library (or the given folder). |
| `--reset-prefix` | Delete every per-game Wine prefix; the library is untouched. |
| `--install-handler`, `--uninstall-handler` | Take over / give back the `oculus://` login link. |
| `login-url` | Print the browser login URL. |
| `--url <callback url>` | Apply a login callback link (what the browser hands back). |
| `--har <file>` | Same, reading the callback out of a browser HAR export. |
| `--settings-page` | Open the window directly on the settings page. |
| `--safe` | Software rendering + X11, for broken or low-VRAM GL. Also `IRIS_SAFE=1`. |
| `--no-covers` | Plain grey cards instead of cover art. Also `IRIS_NO_COVERS=1`. |
| `--x11` | Force the X11 backend (skip Wayland). |

## Notes

- `--launch`, `rescan`, `--download`, `--reset-prefix`, `--install-handler`,
  `--uninstall-handler`, `login-url`, `--url` and `--har` run without a window.
- `--settings-page`, `--safe`, `--no-covers` and `--x11` affect the window run.
- `--safe` and `--no-covers` also work through the environment variables, which
  is how the Xvfb test harness turns covers off without touching the command.

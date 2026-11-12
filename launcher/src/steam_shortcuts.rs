// The Steam protocol path only exists on Linux; on Windows this module keeps
// its pure helpers compileable for tests, with dead-code warnings silenced.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

// Steam integration, safe way only: Steam's own `steam://addnonsteamgame/<path>`
// protocol on a RUNNING client, with a plain .desktop launcher file pointing at
// this binary (--launch <name>). No shortcuts.vdf editing, no icon artwork —
// the desktop client picks the entry up live, and where Steam is not running
// the file stays in the launcher's data dir so a later press (or Steam restart
// followed by a press) adds it. vdf edits would be overwritten by the client
// on exit and are not the safe path.

#[cfg(target_os = "linux")]
use crate::tr;
use crate::{App, GameEntry};
use std::path::PathBuf;

/// Shortcut launcher files live under the app's data dir, not /tmp: temp files
// vanish on reboot and Steam remembers the path it was given.
pub fn shortcut_dir(data_root: &std::path::Path) -> PathBuf {
    data_root.join("steam-shortcuts")
}

pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').to_string()
}

/// Percent-encode everything non-alphanumeric (Steam's handler expects fully
/// quoted paths: steamos-add-to-steam uses urllib.quote(path, safe='')).
pub fn percent_encode(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

pub fn desktop_content(launcher_exe: &str, game_name: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment=Launch with Iris Game Launcher\nExec=\"{}\" --launch \"{}\"\nTerminal=false\nCategories=Game;\n",
        game_name.replace('\n', ""),
        launcher_exe,
        game_name.replace('\n', ""),
    )
}

/// Write the launcher file into the data dir; returns its path. The name in
/// the file is the game's display name; Steam shows it from the desktop
/// entry's Name= line.
pub fn write_shortcut_desktop(
    data_root: &std::path::Path,
    game: &GameEntry,
) -> Result<PathBuf, String> {
    let dir = shortcut_dir(data_root);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create shortcut dir: {e}"))?;
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("locate launcher binary: {e}"))?;
    let path = dir.join(format!("{}.desktop", slug(&game.name)));
    std::fs::write(&path, desktop_content(&exe, &game.name))
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

/// Add one game while asking Steam through the protocol. Needs a running
/// Steam client; the desktop file keeps working after a restart, so every
/// press re-writes it and re-sends the URL.
#[cfg(target_os = "linux")]
pub fn add_to_steam(app: &mut App, idx: usize) {
    let Some(g) = app.store.apps.get(idx).cloned() else {
        app.status = "this game is no longer in the library".into();
        return;
    };
    if !crate::steam_present() {
        app.status = tr(&app.config, "onboard_steam_no_client");
        return;
    }
    let path = match write_shortcut_desktop(app.data_root(), &g) {
        Ok(p) => p,
        Err(e) => {
            app.status = format!("shortcut write failed: {e}");
            return;
        }
    };
    // SteamOS-style sentinel: lets the client add the entry without opening
    // its picker dialog. Harmless on plain desktop Steam.
    let _ = std::fs::write("/tmp/addnonsteamgamefile", b"");
    let url = format!(
        "steam://addnonsteamgame/{}",
        percent_encode(&path.to_string_lossy())
    );
    crate::auth::log_line(&format!("steam add: {}", url));
    let res = std::process::Command::new("steam")
        .arg(&url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .or_else(|_| {
            std::process::Command::new("flatpak")
                .args(["run", "com.valvesoftware.Steam", &url])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        });
    match res {
        Ok(_) => app.status = tr(&app.config, "onboard_steam_queued"),
        Err(e) => app.status = format!("steam command failed: {e}"),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn add_to_steam(app: &mut App, _idx: usize) {
    app.status = "Steam shortcuts are Linux only".into();
}

#[cfg(target_os = "linux")]
pub fn add_installed(app: &mut App) {
    let installed: Vec<usize> = app
        .store
        .apps
        .iter()
        .enumerate()
        .filter(|(_, g)| {
            !g.partial
                && std::path::Path::new(&g.install_dir)
                    .join(&g.launch_exe)
                    .is_file()
        })
        .map(|(i, _)| i)
        .collect();
    if installed.is_empty() {
        app.status = tr(&app.config, "onboard_steam_none_installed");
        return;
    }
    let mut queued = 0usize;
    for i in installed {
        add_to_steam(app, i);
        if app
            .status
            .contains(&tr(&app.config, "onboard_steam_queued"))
        {
            queued += 1;
        }
    }
    app.status = format!("steam shortcuts queued: {queued}");
    crate::auth::log_line(&format!("steam shortcuts queued: {queued}"));
}

#[cfg(not(target_os = "linux"))]
pub fn add_installed(app: &mut App) {
    app.status = "Steam shortcuts are Linux only".into();
}

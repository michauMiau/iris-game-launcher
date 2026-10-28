// Inner attributes must be the very first thing in the file. A cargo-built
// Windows exe is console-subsystem by default, so double-clicking it pops an
// empty black console window that lingers after the app starts. GUI subsystem
// keeps that window away; stdout still works when run from a terminal
// (--download, --url, rescan).
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(test)]
mod tests;

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod auth;
mod fetcher;
mod i18n;

#[derive(Serialize, Deserialize, Default)]
struct Config {
    /// Root directory where store downloads are unpacked (one subdir per app).
    #[serde(default)]
    download_dir: String,
    #[serde(default)]
    prefix_dir: String,
    #[serde(default)]
    #[cfg(not(target_os = "windows"))]
    wine_binary: String,
    /// Directories holding Proton installs (Steam's compatibilitytools.d and
    /// steamapps/common). Used instead of a separately downloaded Wine.
    #[serde(default)]
    #[cfg(not(target_os = "windows"))]
    proton_dirs: Vec<String>,
    /// Full path to the chosen `proton` runscript, empty = fall back to wine_binary.
    #[serde(default)]
    #[cfg(not(target_os = "windows"))]
    proton_selected: String,
    /// Windows only: path to Revive's LibOVRRT DLL (from the ReviveNext or
    /// Revive install). Copied beside the game exe before launch so the game
    /// loads the OpenVR/OpenXR translation layer instead of the vendor one.
    /// No SteamVR registration, no launcher UI from ReviveNext — the layer
    /// alone, as asked.
    #[serde(default)]
    #[cfg(target_os = "windows")]
    revive_dll: String,
    /// Windows only: path to the game's entitlement-check peer, normally
    /// Support\oculus-runtime\OVRServer_x64.exe from the vendor install. Started
    /// (unelevated, no console) before the game when set.
    #[serde(default)]
    #[cfg(target_os = "windows")]
    ovr_server: String,
    /// UI colours: "dark" (how the app looks now), "light", "solarized".
    #[serde(default = "default_theme")]
    theme: String,
    /// UI language: "en" or "pl". Other languages are just new tables in i18n.
    #[serde(default = "default_lang")]
    lang: String,
}

/// First-run UI defaults follow the desktop, not the shipped constants: the
/// locale decides the language, dark/light hints decide the theme. Anything
/// unrecognized falls back to the shipped defaults, so a fresh install on an
/// unknown desktop still opens a working UI.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn lang_from_locale(raw: &str) -> Option<&'static str> {
    let base = raw
        .split(':')
        .next()?
        .split('.')
        .next()?
        .split('_')
        .next()?
        .to_lowercase();
    match base.as_str() {
        "pl" => Some("pl"),
        "en" => Some("en"),
        _ => None,
    }
}

/// `reg query HKCU\Control Panel\International /v Locale` — 0x0415 is
/// Polish-Poland, newer Windows may print a locale name like pl-PL.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn lang_from_reg_locale(output: &str) -> Option<&'static str> {
    let low = output.to_lowercase();
    if low.contains("0x0415") || low.contains("pl-") {
        Some("pl")
    } else if low.contains("0x0409") || low.contains("en-us") {
        Some("en")
    } else {
        None
    }
}

/// Personalize\AppsUseLightTheme: REG_DWORD 0x0 wants dark, 0x1 wants light.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn theme_from_reg_light(output: &str) -> Option<&'static str> {
    let pos = output.find("AppsUseLightTheme")?;
    let tail = output[pos + "AppsUseLightTheme".len()..].trim();
    let val = tail.strip_prefix("REG_DWORD").unwrap_or(tail).trim();
    if val.starts_with("0x1") {
        Some("light")
    } else if val.starts_with("0x0") {
        Some("dark")
    } else {
        None
    }
}

/// `gsettings get org.gnome.desktop.interface color-scheme` answers with
/// 'prefer-dark' or 'prefer-light' quoted.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn theme_from_gnome_scheme(text: &str) -> Option<&'static str> {
    if text.contains("prefer-dark") {
        Some("dark")
    } else if text.contains("prefer-light") {
        Some("light")
    } else {
        None
    }
}

/// KDE kdeglobals carries ColorScheme=... names like Breeze/Dark or Light.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn theme_from_kde_globals(text: &str) -> Option<&'static str> {
    for line in text.lines() {
        let l = line.trim().to_lowercase();
        if l.starts_with("colorscheme=") || l.starts_with("followbehindcolorscheme=") {
            if l.contains("dark") {
                return Some("dark");
            }
            if l.contains("light") {
                return Some("light");
            }
        }
    }
    None
}

/// Language the desktop speaks among the ones the launcher ships (en, pl).
fn system_lang() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = std::process::Command::new("reg")
            .args(["query", r"HKCU\Control Panel\International", "/v", "Locale"])
            .output()
        {
            if let Some(l) = lang_from_reg_locale(&String::from_utf8_lossy(&out.stdout)) {
                return l.into();
            }
        }
        default_lang()
    }
    #[cfg(not(target_os = "windows"))]
    {
        for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Some(l) = lang_from_locale(&std::env::var(var).unwrap_or_default()) {
                return l.into();
            }
        }
        default_lang()
    }
}

/// Theme the desktop prefers among the ones the launcher ships (dark, light).
fn system_theme() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = std::process::Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "/v",
                "AppsUseLightTheme",
            ])
            .output()
        {
            if let Some(t) = theme_from_reg_light(&String::from_utf8_lossy(&out.stdout)) {
                return t.into();
            }
        }
        default_theme()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let gtk = std::env::var("GTK_THEME")
            .unwrap_or_default()
            .to_lowercase();
        if gtk.contains("dark") {
            return "dark".into();
        }
        if gtk.contains("light") {
            return "light".into();
        }
        if let Ok(out) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
        {
            if let Some(t) = theme_from_gnome_scheme(&String::from_utf8_lossy(&out.stdout)) {
                return t.into();
            }
        }
        let home = std::env::var("HOME").unwrap_or_default();
        if let Ok(txt) =
            std::fs::read_to_string(std::path::PathBuf::from(home).join(".config/kdeglobals"))
        {
            if let Some(t) = theme_from_kde_globals(&txt) {
                return t.into();
            }
        }
        default_theme()
    }
}

fn default_theme() -> String {
    "dark".into()
}

fn default_lang() -> String {
    "en".into()
}

/// Steam-like manifest entry, stored in games.json next to config and prefixes.
#[derive(Clone, Serialize, Deserialize)]
struct GameEntry {
    app_id: String,
    name: String,
    install_dir: String,
    launch_exe: String,
    version: String,
    #[serde(default)]
    pub meta_app_id: String,
    #[serde(default)]
    pub binary_id: String,
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub entitled: bool,
    #[serde(default)]
    pub last_played: u64,
    #[serde(default)]
    pub version_code: u64,
    /// Store blurb, empty for folder-scan entries.
    #[serde(default)]
    pub description: String,
    /// Extra command-line arguments appended when launching this game.
    #[serde(default)]
    pub launch_args: Vec<String>,
    /// Extra environment variables ("NAME=value") applied when launching.
    #[serde(default)]
    pub launch_env: Vec<String>,
    /// A download that was cancelled mid-way leaves files on disk but the
    /// game is NOT playable. Marked so the UI can say so instead of lying.
    #[serde(default)]
    pub partial: bool,
    /// Release channels fetched on demand (LIVE, beta, custom), latest binary each.
    #[serde(default)]
    pub branches: Vec<fetcher::Branch>,
    /// Per-game Proton script override; empty = use the global pick. Linux only.
    #[serde(default)]
    pub proton: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Session {
    token: String,
    /// Raw callback params kept until the bearer exchange succeeds, so a failed
    /// redeem can be retried from settings.
    frl_request: String,
    frl_blob: String,
    /// native_sso_token from step 1, used as request_token for blob decrypt.
    #[serde(default)]
    native_sso_token: String,
    /// native_sso_etoken from step 1, tried as request_token fallback.
    #[serde(default)]
    native_sso_etoken: String,
    /// Paste from DevTools if the page's tokens_query rotated the pair.
    #[serde(default)]
    native_sso_manual: String,
    /// Set once a redeem attempt ran for the stored callback (by the handler
    /// process or the GUI). Auto-redeem must not fire again for the same blob.
    #[serde(default)]
    callback_attempted: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct GameStore {
    apps: Vec<GameEntry>,
}

/// Slot the background owned-apps worker writes into; the UI thread merges
/// the result on the next frame.
/// Results carry the bearer they were fetched with: a fetch started for
/// account A must not merge into the library after switching to account B.
type FetchSlot =
    std::sync::Arc<std::sync::Mutex<Option<(String, Result<Vec<fetcher::AppInfo>, String>)>>>;

/// Background result slots: the token used for the request travels with the
/// result, so a stale answer from an old login can never merge into the new one.
type ProbeSlot = std::sync::Arc<std::sync::Mutex<Option<(String, Result<u64, String>)>>>;
type SizeSlot = std::sync::Arc<std::sync::Mutex<Option<(usize, Result<(usize, u64), String>)>>>;

struct App {
    config: Config,
    config_path: PathBuf,
    store_path: PathBuf,
    session_path: PathBuf,
    session: Session,
    /// Exact bytes last read from session.json. The GUI only reloads when
    /// these bytes change, so a bearer pasted into Settings survives the
    /// per-tick refresh instead of being replaced by the old file content.
    session_bytes: Vec<u8>,
    store: GameStore,
    selected: usize,
    /// Index of the game whose dedicated page is open; None = library grid.
    open_game: Option<usize>,
    /// Game index awaiting an uninstall confirmation popup.
    uninstall_ask: Option<usize>,
    status: String,
    settings_page: bool,
    callback_url: String,
    /// Index into the selected game's `branches` vec; 0 before channels fetched.
    selected_branch: usize,
    /// Redeem runs on a background thread so the retry rounds never freeze the
    /// window; the result is picked up in update().
    redeem_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    redeem_inflight: bool,
    fetch_result: FetchSlot,
    fetch_inflight: bool,
    /// Bearer value that already triggered the automatic owned-apps fetch.
    /// Empty = no bearer seen yet; set so the fetch runs once per new token
    /// instead of on every frame.
    /// Token the last owned-apps fetch ran with (button or auto). A manual
    /// fetch counts as handled, so the auto pass cannot fetch a second time
    /// for the same bearer after the manual one lands.
    fetch_token: String,
    /// egui context captured on the first frame; background threads call
    /// request_repaint() through it so results merge without the user
    /// having to wiggle the mouse.
    ctx: Option<egui::Context>,
    /// Live account probe behind the bearer (true "signed in" check).
    /// Account probe results carry the probed bearer, so a probe of an old
    /// token cannot mark the NEW token as verified.
    probe_slot: ProbeSlot,
    probe_inflight: bool,
    probe_token: String,
    verified_account: Option<u64>,
    /// Install popup: estimated size + destination before any bytes move.
    install_prompt: Option<InstallPrompt>,
    /// MelonLoader version popup for one game (per-game installer).
    melon_popup: Option<usize>,
    /// Proton version popup opened from Settings. Windows runs games
    /// natively, so the picker (and its popup) does not exist there.
    #[cfg(not(target_os = "windows"))]
    proton_popup: bool,
    /// The bearer token field is masked until this is pressed.
    token_revealed: bool,
    size_slot: SizeSlot,
    disk_sizes: std::collections::HashMap<String, u64>,
    disk_pending: Option<String>,
    disk_slot: std::sync::Arc<std::sync::Mutex<Option<(String, u64)>>>,
    import_ask: Option<(PathBuf, Option<usize>)>,
    /// Result of the last Windows runtime swap attempt, consumed by the
    /// launch status so a failed swap is not reported as a clean launch.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    win_swap_landed: Option<bool>,
    size_inflight: bool,
    /// Lookup id of the CURRENT size request: a superseded popup's worker can
    /// answer after the newer one, and its size must not be applied to the
    /// new popup (open_install reuses one slot for every popup).
    size_pending_gen: usize,
    dl_slot: Slot<PathBuf>,
    dl_inflight: bool,
    dl_progress: std::sync::Arc<std::sync::Mutex<(usize, usize, u64, u64)>>,
    /// Set when the user presses Cancel; the download thread checks it.
    dl_stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Set while a game process is alive; cleared when it exits or is killed.
    running_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Pid of the spawned game, so Stop kills its whole process tree.
    running_pid: Option<u32>,
    /// Prefix of the running game (Linux). wineserver can escape the process
    /// group, so Stop also kills processes naming this prefix path.
    running_prefix: Option<PathBuf>,
    /// Full exe path of the launched game; pattern matching on it is what
    /// proves the game is alive, because the Proton wrapper pid exits early
    /// while wine children keep running in a detached session.
    running_exe: Option<String>,
    /// Confirm dialog before killing a running game.
    stop_ask: Option<bool>,
    /// Game whose MelonLoader install should add the dll override option.
    melon_pending_idx: Option<usize>,
    /// Per-game Proton picker popup. Linux only, Proton is wine-side.
    #[cfg(not(target_os = "windows"))]
    game_proton_popup: Option<usize>,
    /// Version stamped onto the entry when a download finishes. Captured at
    /// confirm time, not read from the UI mid-flight (the selection can move).
    dl_version: String,
    dl_version_code: u64,
    dl_binary_id: String,
    /// Token edits arrive per keystroke; fetches wait for the token to settle.
    token_snapshot: String,
    token_changed_at: Option<std::time::Instant>,
    /// Decoded covers cached per key; without this every frame re-decodes and
    /// re-uploads every cover PNG (~2 MB each), which is the big CPU cost.
    tex_cache: std::collections::HashMap<String, egui::TextureHandle>,
    /// Plain grey cards instead of cover textures (--no-covers / IRIS_NO_COVERS)
    /// for machines where GPU texture uploads crash or exhaust VRAM.
    covers_off: bool,
    /// The app logo, embedded at compile time, shown beside the title.
    logo_tex: Option<egui::TextureHandle>,
    /// Width/height ratio of the logo asset, kept because egui 0.29
    /// TextureHandle does not expose its dimensions.
    logo_aspect: f32,
    /// Three-dot icon for the game-page options menu. egui's bundled font has
    /// no `⋯` glyph (it rendered as an empty box), so the dots are pixels.
    dots_tex: Option<egui::TextureHandle>,
    /// Folder chooser popup, with what it writes into when "Use this folder".
    dir_popup: Option<(std::path::PathBuf, DirTarget)>,
    /// Game whose folder is being browsed for an extra exe to run in the prefix.
    custom_pick: Option<usize>,
    /// GitHub release lists (Proton GE / MelonLoader) and install slots.
    rel_slot: Slot<Vec<(String, String, String)>>,
    rel_repo: String,
    /// A version-list request that arrived while another repo was
    /// loading; retried once the current list lands.
    pending_repo: Vec<String>,
    rel_inflight: bool,
    proton_releases: Vec<(String, String, String)>,
    melon_releases: Vec<(String, String, String)>,
    #[cfg(not(target_os = "windows"))]
    proton_pick: usize,
    melon_pick: usize,
    #[cfg(not(target_os = "windows"))]
    proton_to_steam: bool,
    install_slot: Slot<String>,
    install_inflight: bool,
    /// Release-channel fetch and integrity check run off the UI thread too;
    /// synchronous HTTP or a hash walk would freeze the window.
    ch_slot: Slot<Vec<fetcher::Branch>>,

    /// Which library entry the running channel fetch belongs to, by app id:
    /// a rescan mid-fetch would otherwise write branches onto the WRONG game.
    ch_key: String,
    ch_inflight: bool,
    chk_slot: Slot<String>,
    chk_inflight: bool,
    /// Game index the running download belongs to (self.selected may change
    /// mid-download when the user clicks another card).
    /// Which library entry the running download belongs to, by app id
    /// rather than by position: a rescan mid-download re-sorts/purges the
    /// vec, and a numeric index would stamp version/install_dir onto a
    /// DIFFERENT game and make the busy gates lie about innocent games.
    dl_key: String,
    /// Set by the capture thread when the game reported a graphics init
    /// failure; the UI turns it into advice (usually Wayland vs X11).
    launch_slot: Slot<String>,
    /// Game whose per-game launch options popup is open.
    tweak_popup: Option<usize>,
    /// Popup text fields, kept between frames so typing survives.
    tweak_args: String,
    tweak_env: String,
    /// Name shown in the top download row while the download is a tool
    /// (Proton / MelonLoader) rather than a game.
    dl_label: String,
}

/// Shared background-result slot used by every worker thread.
type Slot<T> = std::sync::Arc<std::sync::Mutex<Option<Result<T, String>>>>;

/// Pending install popup state.
/// Which field the folder chooser writes into.
#[derive(Clone, Copy, PartialEq)]
enum DirTarget {
    Install,
    DownloadDir,
    PrefixDir,
    Import,
}

#[derive(Clone)]
struct InstallPrompt {
    app_idx: usize,
    binary_id: String,
    channel: String,
    version: String,
    version_code: u64,
    files: usize,
    bytes: u64,
    dir: String,
    sizing: bool,
}

/// One data root for everything: config.json, games.json, prefixes/.
fn data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let parent = if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(x)
    } else if cfg!(target_os = "windows") {
        match std::env::var("LOCALAPPDATA") {
            Ok(a) => PathBuf::from(a),
            Err(_) => PathBuf::from(home),
        }
    } else {
        PathBuf::from(home).join(if cfg!(target_os = "macos") {
            "Library/Application Support"
        } else {
            ".local/share"
        })
    };
    parent.join("iris-launcher")
}

fn token_kind(tok: &str) -> &'static str {
    if tok.is_empty() {
        "none"
    } else if tok.contains('|') {
        "app token - this is not your login token"
    } else if tok.contains('%') {
        "url-encoded token, paste the raw value"
    } else if tok.starts_with("OC") {
        "your login token (OC) - fetch should work"
    } else if tok.starts_with("FRL") {
        "app token - press Login in browser to get a real login token"
    } else {
        "unknown format"
    }
}

#[cfg(not(target_os = "windows"))]
fn steam_compat_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    // Steam lives at ~/.local/share/Steam; ~/.steam/steam is usually a symlink
    // to it. Prefer whichever exists, else create the native location.
    let candidates = [
        PathBuf::from(&home).join(".steam/steam/compatibilitytools.d"),
        PathBuf::from(&home).join(".local/share/Steam/compatibilitytools.d"),
    ];
    for c in &candidates {
        if c.parent().map(|p| p.exists()).unwrap_or(false) {
            return c.clone();
        }
    }
    candidates[1].clone()
}

/// True when a Steam install exists (its compatibilitytools.d parent is
/// real). Without Steam the "install for Steam too" radio must be disabled:
/// install_release does create_dir_all, so choosing it would create a phantom
/// ~/.local/share/Steam/compatibilitytools.d that Steam never reads.
#[cfg(not(target_os = "windows"))]
fn steam_present() -> bool {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    [
        PathBuf::from(&home).join(".steam/steam"),
        PathBuf::from(&home).join(".local/share/Steam"),
    ]
    .iter()
    .any(|p| p.exists())
}

#[cfg(not(target_os = "windows"))]
fn detect_protons(dirs: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(Path::new(d)) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let script = p.join("proton");
            if script.is_file() {
                out.push(script.display().to_string());
            }
        }
    }
    out.sort();
    out
}

/// SteamVR Linux install root usable as PROTON_VR_RUNTIME. Proton's OpenVR
/// bridge loads <root>/bin/linux64/vrclient.so and only connects to a vrserver
/// that is already running; it never starts one itself (Valve issue #962).
#[cfg(not(target_os = "windows"))]
fn steamvr_runtime_root(home: &str) -> Option<std::path::PathBuf> {
    [
        std::path::PathBuf::from(format!("{home}/.steam/steam/steamapps/common/SteamVR")),
        std::path::PathBuf::from(format!(
            "{home}/.local/share/Steam/steamapps/common/SteamVR"
        )),
    ]
    .into_iter()
    .find(|root| root.join("bin/linux64/vrclient.so").is_file())
}

#[cfg_attr(target_os = "windows", allow(dead_code))]
/// GE-Proton since 10-33 initializes the VR registry inside wineopenxr, so XR
/// works outside Steam. Valve-based builds (Valve Proton, proton-cachyos) only
/// get `HKCU\\Software\\Wine\\VR` from steam_helper, which runs for Steam-launched
/// games; outside Steam wineopenxr stays dead and every xr call returns
/// XR_ERROR_RUNTIME_UNAVAILABLE (Valve issues #7382 and #8256). A build whose
/// name does not parse as GE-Proton >= 10-33 cannot open an XR session.
fn proton_vr_capable(script: &std::path::Path) -> bool {
    let name = script
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let Some(rest) = name.strip_prefix("GE-Proton") else {
        return false;
    };
    let mut parts = rest.split('-');
    let Some(major) = parts.next().and_then(|v| v.parse::<u32>().ok()) else {
        return false;
    };
    let minor = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    major > 10 || (major == 10 && minor >= 33)
}

/// protonfixes (GE-Proton) aborts the whole launch when no numeric game id
/// is in the environment, so every Proton run gets one: the store app id if
/// it is numeric, otherwise a synthetic 10-digit hash far outside the real
/// Steam app id space so no other game's fixes can match.
#[cfg(not(target_os = "windows"))]
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn steam_app_id_for(name: &str, app_id: &str) -> String {
    let digits: String = app_id.chars().filter(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        return digits;
    }
    let mut h: u64 = 0xcbf29ce4842223b6;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (h % 9_000_000_000 + 1_000_000_000).to_string()
}

/// SteamVR ships its OpenXR runtime manifest next to the install root
/// (steamxr_linux64.json, library_path = bin/linux64/vrclient.so). XR_RUNTIME_JSON
/// is the loader's highest-priority override, which matters inside Wine: the
/// loader would otherwise look in the prefix's XDG dirs and never see the host
/// manifest, so Unity's OpenXR plugin reports XR_ERROR_RUNTIME_UNAVAILABLE even
/// with SteamVR running (Valve SteamVR-for-Linux issue #840).
/// OpenXR manifest discovery used on Windows: SteamVR ships its runtime
/// manifest in the install root; XR_RUNTIME_JSON overrides the registry lookup,
/// so the launcher points at whichever json declares a library_path.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn xr_manifest(root: &std::path::Path) -> Option<std::path::PathBuf> {
    for e in std::fs::read_dir(root).ok()?.flatten() {
        let path = e.path();
        if path.extension().map(|x| x == "json").unwrap_or(false) {
            if let Ok(txt) = std::fs::read_to_string(&path) {
                if txt.contains("library_path") {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Parse `reg query HKLM\SOFTWARE\Valve\Steam /v InstallPath` output.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn steam_install_path_from_reg(output: &str) -> Option<String> {
    for line in output.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("InstallPath") {
            let rest = rest.trim().strip_prefix("REG_SZ").unwrap_or(rest).trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

/// `tasklist /FI "IMAGENAME eq x.exe"` prints the image name when it matches,
/// and "no tasks matched" otherwise.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn tasklist_has_image(output: &str, image: &str) -> bool {
    output
        .to_ascii_lowercase()
        .contains(&image.to_ascii_lowercase())
}

#[cfg(not(target_os = "windows"))]
fn steamxr_manifest(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let m = root.join("steamxr_linux64.json");
    m.is_file().then_some(m)
}

/// Cached 2 s: launch gating and status strip ask often, and pgrep/tasklist
/// are too slow to spawn per frame. Freshness within two seconds is plenty
/// for "is SteamVR's vrserver up".
#[cfg(not(target_os = "windows"))]
fn vrserver_running_cached() -> bool {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    static VAL: AtomicU64 = AtomicU64::new(0);
    let now = now_unix();
    let last = LAST.load(Ordering::Relaxed);
    if now.saturating_sub(last) >= 2 {
        let v = vrserver_running();
        VAL.store(v as u64, Ordering::Relaxed);
        LAST.store(now, Ordering::Relaxed);
    }
    VAL.load(Ordering::Relaxed) != 0
}

#[cfg(not(target_os = "windows"))]
fn vrserver_running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "vrserver"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
/// True when a pid still exists. Zombies answer yes until the launch watcher
/// reaps them, which is exactly when the flag clears too.
fn pid_alive(pid: u32) -> bool {
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|st| st.success())
            .unwrap_or(false)
    }
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }
}

/// Mirror the host OpenVR path registry (~/.config/openvr, the vrpathreg json
/// naming the runtime, config and log dirs) into the Wine prefix home so
/// Proton's vrclient sees the same paths a native app sees. This is OpenVR,
/// not OpenXR; the OpenXR runtime is pointed via XR_RUNTIME_JSON separately.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn openvr_pathreg_step(prefix: &std::path::Path) -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let src = std::path::Path::new(&home).join(".config/openvr");
    if !src.is_dir() {
        return Vec::new();
    }
    let mut placed = Vec::new();
    let files: Vec<(std::ffi::OsString, Vec<u8>)> = std::fs::read_dir(src)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|f| f.path().is_file())
        .filter_map(|f| {
            let data = std::fs::read(f.path()).ok()?;
            Some((f.path().file_name()?.to_owned(), data))
        })
        .collect();
    // Wine HOME is drive_c/users/<user>; mirror every registry file there.
    let users = prefix.join("drive_c/users");
    let Ok(entries) = std::fs::read_dir(&users) else {
        return Vec::new();
    };
    for user in entries.filter_map(|e| e.ok()) {
        let dst = user.path().join(".config/openvr");
        if std::fs::create_dir_all(&dst).is_err() {
            continue;
        }
        for (name, data) in &files {
            let target = dst.join(name);
            // Skip when the mirror already holds identical bytes.
            if std::fs::read(&target)
                .map(|old| old == *data)
                .unwrap_or(false)
            {
                continue;
            }
            if std::fs::write(&target, data).is_ok() {
                placed.push(target);
            }
        }
    }
    placed
}

/// Diagnostic: does the live vrserver share this process' network namespace,
/// and are its /steamvr abstract sockets visible here? SteamVR pipes are
/// abstract AF_UNIX sockets ("/steamvr/SteamVR_Namespace"), which live per
/// network namespace: a vrserver started inside a container the game is not
/// in is invisible no matter what env vars are set.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn vr_netns_diag() {
    let out = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("vrserver")
        .output();
    let Ok(o) = out else { return };
    let stdout = String::from_utf8_lossy(&o.stdout);
    let Some(pid) = stdout.lines().next() else {
        return;
    };
    let server_ns = std::fs::read_link(format!("/proc/{pid}/ns/net"))
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let own_ns = std::fs::read_link("/proc/self/ns/net")
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    auth::log_line(&format!(
        "vrserver netns {} vs launcher netns {} — {}",
        server_ns,
        own_ns,
        if server_ns == own_ns {
            "same namespace, abstract pipes reachable"
        } else {
            "different namespaces, abstract pipes NOT reachable from the game"
        }
    ));
    if let Ok(ss) = std::process::Command::new("ss").args(["-xlpn"]).output() {
        let n = String::from_utf8_lossy(&ss.stdout)
            .lines()
            .filter(|l| l.contains("steamvr"))
            .count();
        auth::log_line(&format!("steamvr sockets visible here: {n}"));
    }
}

/// VR_RUNTIME_DIR straight from the live vrserver's /proc environ, kept only
/// if the pipe files are visible from here. Steam runs vrserver inside its
/// runtime container and sets VR_RUNTIME_DIR to a dir that stays invisible
/// outside, which is why launching SteamVR through Steam leaves the game at
/// XR_ERROR_RUNTIME_UNAVAILABLE while launching SteamVR's own desktop handler
/// puts pipes in ~/.config/SteamVR where the game can reach them.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn vr_runtime_dir_from_server_env() -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("vrserver")
        .output()
        .ok()?;
    let pid = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    let env = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    for kv in env.split(|b| *b == 0) {
        let text = String::from_utf8_lossy(kv);
        let dir = std::path::PathBuf::from(text.strip_prefix("VR_RUNTIME_DIR=")?);
        let visible = std::fs::read_dir(&dir)
            .ok()
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().contains("pipe"))
            })
            .unwrap_or(false);
        // Log what the server advertises even when it is unusable, so the
        // next launcher.log run names Steam's real pipe dir instead of
        // guessing whether the container hides it.
        auth::log_line(&format!(
            "live vrserver advertises VR_RUNTIME_DIR {} — {}",
            dir.display(),
            if visible {
                "pipes visible from here"
            } else {
                "no pipe files visible from here"
            }
        ));
        if visible {
            return Some(dir);
        }
    }
    None
}

#[cfg_attr(target_os = "windows", allow(dead_code))]
fn vr_runtime_dir_discovered() -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("ss")
        .args(["-xlpn"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if !line.contains("vrserver") {
            continue;
        }
        for tok in line.split_whitespace() {
            if tok.starts_with('/') && (tok.contains("vrserverpipe") || tok.ends_with("pipe")) {
                return std::path::Path::new(tok).parent().map(|d| d.to_path_buf());
            }
        }
    }
    None
}

/// Open a folder (or file) in the OS file browser.
/// One Wine/Proton prefix per game under the configured prefix root
/// (prefix_dir/<folder>). Windows runs exes directly, so the prefix machinery
/// is Linux-only, but --reset-prefix uses this root on every platform.
fn game_prefix_root(prefix_dir: &str) -> std::path::PathBuf {
    if prefix_dir.trim().is_empty() {
        data_dir().join("prefixes")
    } else {
        PathBuf::from(prefix_dir.trim())
    }
}

/// Proton runscripts outside Steam abort with "No compat data path?" unless
/// both STEAM_COMPAT variables point at a real prefix directory.
/// Wine's Z: drive follows the process cwd, so launches set cwd to the game
/// folder (measured: dir Z lists only game files).
#[cfg(not(target_os = "windows"))]
fn prefix_env(cmd: &mut std::process::Command, prefix: &Path) {
    cmd.env("WINEPREFIX", prefix);
    cmd.env("STEAM_COMPAT_DATA_PATH", prefix);
    cmd.env(
        "STEAM_COMPAT_CLIENT_INSTALL_PATH",
        std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()),
    );
}

/// Place the platform shim as LibOVRPlatform64_1.dll beside the game
/// exe. Unity's platform wrapper loads that DLL by name, and the
/// Bonelab Player.log shows what happens without it: DllNotFoundException in
/// MarrowEntitlement, "Entitlement Failed", then a NullReferenceException in
/// SceneBootstrapper that kills the game right after the splash. The shim
/// answers the entitlement call as entitled offline, no vendor runtime needed.
fn platform_shim_step(game_dir: &Path) -> Option<PathBuf> {
    if !game_dir.is_dir() {
        return None;
    }
    let data = include_bytes!("../../assets/platform-shim.dll");
    let target = game_dir.join("LibOVRPlatform64_1.dll");
    if target.is_file() {
        // Skip writes only when the file already matches the shipped shim;
        // an exists() check alone kept old shims alive forever, so games ran
        // with pre-timeline DLLs even after launcher updates.
        if let Ok(cur) = std::fs::read(&target) {
            if cur.len() == data.len() && cur[..] == data[..] {
                return None;
            }
        }
    }
    std::fs::write(&target, data).ok().map(|_| target)
}

/// Place Revive's OpenXR-backed LibOVRRT replacement beside the exe as
/// LibOVRRT64_1.dll, the same file the Windows installer registers. It exports
/// the ovr_* API the headset plugin expects and talks OpenXR underneath, which
/// reaches the SteamVR Linux runtime through Proton's OpenXR passthrough.
const REVIVE_XR_BYTES: &[u8] = include_bytes!("../../assets/LibReviveXR64.dll");

#[cfg(not(target_os = "windows"))]
fn librevive_rt_step(game_dir: &Path) -> Option<PathBuf> {
    let target = game_dir.join("LibOVRRT64_1.dll");
    if !game_dir.is_dir() || target.exists() {
        return None;
    }
    std::fs::write(&target, REVIVE_XR_BYTES)
        .ok()
        .map(|_| target)
}

/// SteamVR install on Windows: registry InstallPath first, common folders as
/// fallback. The install is recognized by bin\win64\vrserver.exe.
#[cfg(target_os = "windows")]
fn steamvr_root_windows() -> Option<std::path::PathBuf> {
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(out) = std::process::Command::new("reg")
        .args(["query", r"HKLM\SOFTWARE\Valve\Steam", "/v", "InstallPath"])
        .output()
    {
        if let Some(p) = steam_install_path_from_reg(&String::from_utf8_lossy(&out.stdout)) {
            roots.push(std::path::PathBuf::from(p));
        }
    }
    if let Ok(pf) = std::env::var("ProgramFiles(x86)") {
        roots.push(std::path::PathBuf::from(pf).join("Steam"));
    }
    if let Ok(pf) = std::env::var("ProgramFiles") {
        roots.push(std::path::PathBuf::from(pf).join("Steam"));
    }
    roots.into_iter().find(|r| {
        r.join(r"steamapps\common\SteamVR\bin\win64\vrserver.exe")
            .is_file()
    })
}

#[cfg(target_os = "windows")]
fn vrserver_running_windows_cached() -> bool {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    static VAL: AtomicU64 = AtomicU64::new(0);
    let now = now_unix();
    if now.saturating_sub(LAST.load(Ordering::Relaxed)) >= 2 {
        VAL.store(vrserver_running_windows() as u64, Ordering::Relaxed);
        LAST.store(now, Ordering::Relaxed);
    }
    VAL.load(Ordering::Relaxed) != 0
}

#[cfg(target_os = "windows")]
fn vrserver_running_windows() -> bool {
    match std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq vrserver.exe"])
        .output()
    {
        Ok(out) => tasklist_has_image(&String::from_utf8_lossy(&out.stdout), "vrserver.exe"),
        Err(_) => false,
    }
}

/// The vendor runtime folder the Unity headset plugin resolves through its own
/// path lookup: it loads LibOVRRTImpl64_1.dll from there even when the game
/// folder holds a replacement, which is why beside-exe copies never redirected
/// Boneworks. Replacing the Impl here redirects every plugin game on the
/// machine to the OpenXR translation layer, i.e. the launcher replaces the
/// headset link runtime slot wholesale.
#[cfg(target_os = "windows")]
fn meta_runtime_dir() -> Option<std::path::PathBuf> {
    const CANDIDATES: [&str; 3] = [
        r"D:\Oculus\Support\oculus-runtime",
        r"C:\Program Files\Oculus\Support\oculus-runtime",
        r"C:\Oculus\Support\oculus-runtime",
    ];
    CANDIDATES
        .iter()
        .map(std::path::PathBuf::from)
        .find(|d| d.join("LibOVRRTImpl64_1.dll").is_file() || d.join("LibOVRRT64_1.dll").is_file())
}

/// True when the runtime file already matches the translation DLL byte-for-byte.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn dll_matches(path: &std::path::Path, len: u64) -> bool {
    path.is_file() && path.metadata().map(|m| m.len() == len).unwrap_or(false)
}

/// Run a PowerShell script elevated (one UAC prompt), for writes into Program
/// Files / drive D that an unelevated launcher cannot do.
#[cfg(target_os = "windows")]
fn run_elevated_ps1(script_path: &std::path::Path) -> bool {
    std::process::Command::new("powershell")
        .args([
            "-NonInteractive",
            "-Command",
            &format!(
                "Start-Process -Verb RunAs -FilePath powershell -ArgumentList '-NonInteractive','-ExecutionPolicy','Bypass','-File','{}' -WindowStyle Hidden",
                script_path.display()
            ),
        ])
        .spawn()
        .is_ok()
}

#[cfg(target_os = "windows")]
fn meta_runtime_swap(sources: &[(&str, &str)]) -> bool {
    // sources: (source dll, destination dll) pairs; originals are backed up
    // before the first swap so the Restore button can put them back.
    let mut script = String::new();
    for (src, dst) in sources {
        script.push_str(&format!(
            "Copy-Item -LiteralPath \"{}\" -Destination \"{}\" -Force\r\n",
            src, dst
        ));
    }
    let ps1 = data_dir().join("iris-runtime-swap.ps1");
    std::fs::write(&ps1, script).is_ok() && run_elevated_ps1(&ps1)
}

/// tools dir; it hooks game processes so the translation DLLs load inside
/// them. Detached, the game owns its own lifetime afterwards.
#[cfg(target_os = "windows")]
fn revive_injector_start(tools_dir: &Path) -> Option<u32> {
    let exe = tools_dir.join("ReviveInjector.exe");
    if !exe.is_file() {
        return None;
    }
    let child = std::process::Command::new(&exe)
        .current_dir(tools_dir)
        .spawn()
        .ok()?;
    Some(child.id())
}

/// Copy the Revive translation DLL beside the game exe. Windows DLL lookup
/// checks the exe's own folder before System32, so a copy there redirects the
/// game to Revive's LibOVR. Returns the copied path, None when unusable.
#[cfg(target_os = "windows")]
fn revive_dll_step(revive_dll: &str, game_dir: &Path) -> Option<PathBuf> {
    let dll = Path::new(revive_dll.trim());
    if !dll.is_file() || !game_dir.is_dir() {
        return None;
    }
    let name = dll.file_name()?;
    let target = game_dir.join(name);
    if dll == target {
        return None;
    }
    std::fs::copy(dll, &target).ok().map(|_| target)
}

/// Find the Revive translation DLL and the vendor runtime peer on this machine
/// after an install (ReviveNext setup is an admin NSIS installer, so the user
/// runs it; the launcher locates what it dropped). Scans the usual install
/// roots, at a bounded depth so it never walks the whole drive.
#[cfg(target_os = "windows")]
fn revive_scan(roots: &[std::path::PathBuf]) -> (Option<PathBuf>, Option<PathBuf>) {
    let dll_names = [
        "libovrrt64_1.dll",
        "libovrrt.dll",
        "libovr.dll",
        "libovrrt64.dll",
    ];
    let mut dll = None;
    let mut server = None;
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let mut stack = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            if depth > 4 {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push((path, depth + 1));
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if dll.is_none() && dll_names.contains(&name.as_str()) {
                    dll = Some(path.clone());
                }
                if server.is_none() && name == "ovrserver_x64.exe" {
                    server = Some(path.clone());
                }
            }
        }
    }
    (dll, server)
}

/// Standard Windows install roots worth scanning for the Revive layer.
#[cfg(target_os = "windows")]
fn revive_scan_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        roots.push(PathBuf::from(pf));
    }
    if let Ok(pf) = std::env::var("ProgramFiles(x86)") {
        roots.push(PathBuf::from(pf));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        roots.push(PathBuf::from(local));
    }
    roots
}

/// Cold-start the vendor runtime peer the game's entitlement check talks to.
/// Unelevated, no console window; the process is left running detached.
#[cfg(target_os = "windows")]
fn revive_server_start(ovr_server: &str) -> Option<u32> {
    let exe = Path::new(ovr_server.trim());
    if !exe.is_file() {
        return None;
    }
    let mut cmd = std::process::Command::new(exe);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    cmd.stdin(std::process::Stdio::null());
    cmd.spawn().ok().map(|c| c.id())
}

/// Every .exe in a game folder, minus compatibility junk.
fn exe_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let fp = e.path();
            if fp.is_dir() {
                let name = fp
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if name.contains("wine")
                    || name.contains("proton")
                    || name.contains("compatibility")
                {
                    continue;
                }
                stack.push(fp);
                continue;
            }
            if !fp.extension().map(|x| x == "exe").unwrap_or(false) {
                continue;
            }
            let n = fp
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if n.contains("crashhandler")
                || n.contains("setup")
                || n.contains("wine")
                || n.contains("proton")
            {
                continue;
            }
            out.push(fp);
        }
    }
    out.sort();
    out
}

/// Ask the desktop's own folder/file picker (GNOME, KDE, Yad, PowerShell on
/// Windows). Returns Some(Some(path)) when a path was chosen, Some(None)
/// when a dialog opened and the user cancelled it, and None when no dialog
/// tool exists here so the caller falls back to the built-in chooser.
fn system_pick(start: &Path, want_dir: bool) -> Option<Option<PathBuf>> {
    #[cfg(target_os = "windows")]
    {
        // PowerShell's WinForms dialogs are the real Windows dialogs.
        let script = if want_dir {
            format!(
                "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.SelectedPath = '{}'; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $d.SelectedPath }}",
                start.display()
            )
        } else {
            format!(
                "Add-Type -AssemblyName System.Windows.Forms; $o = New-Object System.Windows.Forms.OpenFileDialog; $o.InitialDirectory = '{}'; $o.Filter = 'Executable files (*.exe)|*.exe|All files (*.*)|*.*'; if ($o.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $o.FileName }}",
                start.display()
            )
        };
        let out = match auth::quiet_powershell(&script) {
            Ok(o) => o,
            Err(_) => return None,
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let Some(picked) = text.lines().map(|l| l.trim()).find(|l| !l.is_empty()) else {
            return Some(None); // dialog ran, nothing chosen (cancelled)
        };
        let path = PathBuf::from(picked);
        if want_dir {
            if path.is_dir() {
                Some(Some(path))
            } else {
                Some(None)
            }
        } else {
            if path.is_file() {
                Some(Some(path))
            } else {
                Some(None)
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        // GNOME (zenity), KDE (kdialog), Yad. Each prints the chosen path.
        let tries: Vec<(&str, Vec<String>)> = if want_dir {
            vec![
                (
                    "zenity",
                    vec![
                        "--file-selection".into(),
                        "--directory".into(),
                        "--title=Choose a folder".into(),
                    ],
                ),
                (
                    "kdialog",
                    vec!["--getexistingdirectory".into(), start.display().to_string()],
                ),
                (
                    "yad",
                    vec!["--directory".into(), "--title=Choose a folder".into()],
                ),
            ]
        } else {
            vec![
                (
                    "zenity",
                    vec!["--file-selection".into(), "--title=Choose a file".into()],
                ),
                (
                    "kdialog",
                    vec![
                        "--getopenfilename".into(),
                        start.display().to_string(),
                        "*.exe|Executables".into(),
                    ],
                ),
                ("yad", vec!["--file".into(), "--title=Choose a file".into()]),
            ]
        };
        for (tool, args) in tries {
            // A missing tool must not abort the chain: zenity absent on
            // Fedora/KDE boxes used to kill the native picker at the first
            // spawn and silently fall to the built-in chooser.
            let out = match std::process::Command::new(tool).args(&args).output() {
                Ok(o) => o,
                Err(_) => continue,
            };
            if !out.status.success() {
                return Some(None); // dialog opened; user cancelled or closed it
            }
            let text = String::from_utf8_lossy(&out.stdout);
            let Some(picked) = text.lines().map(|l| l.trim()).find(|l| !l.is_empty()) else {
                return Some(None);
            };
            let path = PathBuf::from(picked);
            if (want_dir && path.is_dir()) || (!want_dir && path.is_file()) {
                return Some(Some(path));
            }
            return Some(None);
        }
        None
    }
}

/// Translate a UI key in the user's language. Keys without a translation
/// fall back to English — adding a language later is another table in i18n.
fn tr(cfg: &Config, key: &str) -> String {
    i18n::fill(&i18n::text(i18n::parse(&cfg.lang), key), &[])
}

/// UI colours: dark is the shipped look, light is egui's light theme,
/// solarized is the classic blue-on-dark palette.
fn apply_theme(ctx: &egui::Context, theme: &str) {
    match theme {
        "light" => ctx.set_visuals(egui::Visuals::light()),
        "solarized" => {
            // Solarized dark: base03 background, base02 panels, blue accent.
            let text = egui::Color32::from_rgb(238, 232, 213);
            let hint = egui::Color32::from_rgb(133, 154, 150);
            let base03 = egui::Color32::from_rgb(0, 43, 54);
            let base02 = egui::Color32::from_rgb(7, 54, 66);
            let base1 = egui::Color32::from_rgb(88, 110, 117);
            let blue = egui::Color32::from_rgb(38, 139, 210);
            let mut v = egui::Visuals::dark();
            v.override_text_color = Some(text);
            v.hyperlink_color = blue;
            v.faint_bg_color = base02;
            v.extreme_bg_color = base03;
            v.widgets.noninteractive.bg_fill = base03;
            v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, base1);
            v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, text);
            v.widgets.inactive.bg_fill = base02;
            v.widgets.inactive.weak_bg_fill = base02;
            v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, text);
            v.widgets.hovered.bg_fill = base1;
            v.widgets.hovered.weak_bg_fill = base1;
            v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, text);
            v.widgets.active.bg_fill = blue;
            v.widgets.active.weak_bg_fill = blue;
            v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, text);
            v.selection.bg_fill = blue;
            v.selection.stroke = egui::Stroke::new(1.0_f32, hint);
            ctx.set_visuals(v);
        }
        _ => ctx.set_visuals(egui::Visuals::dark()),
    }
}

/// winecfg is a separate binary, not an argument to wine. Find a real one:
/// next to the configured wine path first, then Wine/winecfg from PATH.
#[cfg(not(target_os = "windows"))]
/// Resolve the wine settings tool. Standalone winecfg first, then wine on
/// PATH, then the wine bundled inside the selected Proton (Proton ships
/// winecfg.exe, and many Wayland installs have no standalone wine at all).
/// Returns (executable, args); None means no tool exists on this machine.
fn wine_settings_plan(wine_binary: &str, proton: &str) -> Option<(String, Vec<String>)> {
    let wb = std::path::Path::new(wine_binary.trim());
    if wb.is_file() {
        if let Some(dir) = wb.parent() {
            let cfg = dir.join("winecfg");
            if cfg.is_file() {
                return Some((cfg.display().to_string(), vec![]));
            }
        }
    }
    for name in ["winecfg", "wine"] {
        for dir in std::env::var("PATH").unwrap_or_default().split(':') {
            let cand = std::path::Path::new(dir).join(name);
            if cand.is_file() {
                return Some((cand.display().to_string(), vec![]));
            }
        }
    }
    let p = std::path::Path::new(proton.trim());
    if !p.as_os_str().is_empty() {
        let dir = if p.is_file() {
            p.parent().unwrap_or(std::path::Path::new("."))
        } else {
            p
        };
        let wrapper = dir.join("files/bin/wine");
        if wrapper.is_file() {
            return Some((wrapper.display().to_string(), vec!["winecfg.exe".into()]));
        }
    }
    None
}

/// Per-game launch knobs: extra args and NAME=value env vars, applied before
/// the game executable is spawned.
fn apply_launch_tweak_env(cmd: &mut std::process::Command, g: &GameEntry) {
    for pair in &g.launch_env {
        if let Some((k, v)) = pair.split_once('=') {
            if !k.trim().is_empty() {
                cmd.env(k.trim(), v);
            }
        }
    }
}

/// Extra arguments, in the order the game expects them (after the exe).
fn launch_tweak_args(g: &GameEntry) -> Vec<String> {
    g.launch_args
        .iter()
        .filter(|a| !a.trim().is_empty())
        .map(|a| a.trim().to_string())
        .collect()
}

/// Verdict for the update check: plain words, no channel jargon. Pure so the
/// three outcomes can be unit-tested.
fn update_verdict(g: &GameEntry, branch: usize) -> String {
    if g.install_dir.is_empty() {
        return "not installed yet — nothing to update".to_string();
    }
    match g.branches.get(branch) {
        Some(b) if b.version_code > g.version_code => {
            format!("update available — {} ({})", b.name, b.version)
        }
        Some(_) => "no update available — this build is the newest".to_string(),
        None => "no store channels yet — press Check updates first".to_string(),
    }
}

/// Unity's data folder: inside a Wine prefix on Linux (drive_c/users/<user>/
/// AppData/LocalLow, Proton names the user steamuser), and LOCALAPPDATA\LocalLow
/// on native Windows. Both hold the company folder with saves and Player.log.
fn data_folder(prefix: Option<&Path>) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let _ = prefix;
        let low = PathBuf::from(std::env::var("LOCALAPPDATA").ok()?);
        let low = low.join("LocalLow");
        if !low.exists() {
            std::fs::create_dir_all(&low).ok()?;
        }
        Some(low)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let prefix = prefix?;
        // Proton builds its prefix inside a pfx subfolder of the compat data
        // dir (prefixes/<game>/pfx/drive_c/...), plain Wine puts drive_c
        // straight under the prefix. Both layouts must be searched or the
        // data folder button claims "not there yet" while the files exist.
        let roots = [
            prefix.join("drive_c").join("users"),
            prefix.join("pfx").join("drive_c").join("users"),
        ];
        let user = roots
            .iter()
            .filter_map(|users| {
                std::fs::read_dir(users).ok()?.find_map(|e| {
                    let p = e.ok()?.path();
                    p.is_dir().then_some(p)
                })
            })
            .next()?;
        let low = user.join("AppData").join("LocalLow");
        if !low.exists() {
            std::fs::create_dir_all(&low).ok()?;
        }
        Some(low)
    }
}

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[derive(Clone, Copy)]
enum Preset {
    Dll(&'static str),
    Arg(&'static str),
    Env(&'static str),
    /// Adds PROTON_VR_RUNTIME pointing at the SteamVR Linux install.
    #[cfg(not(target_os = "windows"))]
    SteamVr,
}

/// Lines of a launch_env field, key=value form.
fn env_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Wine joins override entries with commas, and an order spec like
/// "version=n,b" contains commas itself, so entries are matched at boundary
/// positions instead of naive comma splitting.
fn override_contains(val: &str, item: &str) -> bool {
    val == item
        || val.starts_with(&format!("{item},"))
        || val.ends_with(&format!(",{item}"))
        || val.contains(&format!(",{item},"))
}

/// Drop one entry from an override value, keeping the neighbours intact.
fn override_remove(val: &str, item: &str) -> Option<String> {
    if val == item {
        return None;
    }
    if val.starts_with(&format!("{item},")) {
        return Some(val[item.len() + 1..].to_string());
    }
    if let Some(pos) = val.find(&format!(",{item}")) {
        let mut out = val[..pos].to_string();
        out.push_str(&val[pos + item.len() + 1..]);
        return Some(out);
    }
    Some(val.to_string())
}

fn has_dll_override(env: &[String], item: &str) -> bool {
    env.iter().any(|l| {
        l.starts_with("WINEDLLOVERRIDES=")
            && override_contains(l["WINEDLLOVERRIDES=".len()..].trim(), item)
    })
}

/// Add or remove one item inside WINEDLLOVERRIDES without touching other
/// entries in the same line or other env lines. Returns true when it changed.
fn toggle_dll_override(env: &mut Vec<String>, item: &str, on: bool) -> bool {
    let idx = env.iter().position(|l| l.starts_with("WINEDLLOVERRIDES="));
    if on {
        match idx {
            Some(i) => {
                let val = env[i]["WINEDLLOVERRIDES=".len()..].trim();
                if override_contains(val, item) {
                    return false;
                }
                env[i] = if val.is_empty() {
                    format!("WINEDLLOVERRIDES={item}")
                } else {
                    format!("WINEDLLOVERRIDES={val},{item}")
                };
            }
            None => env.push(format!("WINEDLLOVERRIDES={item}")),
        }
        true
    } else {
        let Some(i) = idx else { return false };
        match override_remove(env[i]["WINEDLLOVERRIDES=".len()..].trim(), item) {
            Some(rest) => env[i] = format!("WINEDLLOVERRIDES={rest}"),
            None => {
                env.remove(i);
            }
        }
        true
    }
}

/// Add or remove a whole key=value env entry in the launch_env field.
fn toggle_env_entry(env: &mut Vec<String>, key: &str, value: &str, on: bool) -> bool {
    if on {
        if env.iter().any(|l| l.starts_with(&format!("{key}="))) {
            return false;
        }
        env.push(format!("{key}={value}"));
        true
    } else {
        let before = env.len();
        env.retain(|l| !l.starts_with(&format!("{key}=")));
        env.len() != before
    }
}

fn has_env_key(env: &[String], key: &str) -> bool {
    env.iter().any(|l| l.starts_with(&format!("{key}=")))
}

fn has_arg(args: &[String], arg: &str) -> bool {
    args.iter().any(|a| a.trim() == arg)
}

/// Add or remove one command-line arg without touching the other lines.
fn toggle_arg(args: &mut Vec<String>, arg: &str, on: bool) -> bool {
    if on {
        if args.iter().any(|a| a.trim() == arg) {
            return false;
        }
        args.push(arg.to_string());
        true
    } else {
        let before = args.len();
        args.retain(|a| a.trim() != arg);
        args.len() != before
    }
}

fn open_folder(path: &Path) {
    let _ = std::process::Command::new(if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    })
    .arg(path)
    .spawn();
}

/// Reload session.json into memory only when the bytes on disk differ from
/// what we last read or wrote. Comparing loaded fields every tick used to
/// wipe a bearer pasted into the Settings field: an untouched old file still
/// differed from memory, so it replaced the paste before any fetch could use
/// it. Returns true when the loaded fields changed.
fn apply_disk_session(session: &mut Session, remembered: &mut Vec<u8>, path: &Path) -> bool {
    let Ok(b) = std::fs::read(path) else {
        return false;
    };
    if b == *remembered {
        return false;
    }
    *remembered = b.clone();
    let Ok(s) = serde_json::from_slice::<Session>(&b) else {
        return false;
    };
    // The handler writes the SSO pair too; without these in the comparison
    // an externally-rewritten session.json would never reach the Apply path.
    let changed = s.token != session.token
        || s.frl_request != session.frl_request
        || s.frl_blob != session.frl_blob
        || s.callback_attempted != session.callback_attempted
        || s.native_sso_manual != session.native_sso_manual
        || s.native_sso_token != session.native_sso_token
        || s.native_sso_etoken != session.native_sso_etoken;
    if changed {
        *session = s;
    }
    changed
}

/// Truthful signed-in state as (text, tone). Green requires BOTH a token in
/// memory and a probe that verified it; verified alone used to survive
/// Forget token. The old check/warning glyphs came out as empty squares in
/// egui's bundled font, so the icon is painted by hand instead.
fn marker(signed: bool, verified: bool, probe_inflight: bool) -> (&'static str, &'static str) {
    if verified && signed {
        ("signed in", "ok")
    } else if probe_inflight && signed {
        ("checking account", "wait")
    } else if signed {
        ("token not verified", "warn")
    } else {
        ("not signed in", "off")
    }
}

/// Hand-drawn state icon: egui's default font has no check/exclamation glyphs
/// (they rendered as squares), so draw them ourselves.
/// Truncates text to fit `width` px (roughly 7 px per char) so it cannot
/// overlap whatever sits to its left.
fn fit_text(text: &str, width: f32) -> String {
    let per_char = 7.2_f32;
    let fit = (width / per_char).floor() as usize;
    if fit == 0 {
        return "…".to_string();
    }
    if text.chars().count() <= fit {
        return text.to_string();
    }
    let cut: String = text.chars().take(fit - 1).collect();
    format!("{cut}…")
}

fn state_icon(ui: &mut egui::Ui, tone: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    match tone {
        "ok" => {
            p.circle(
                c,
                6.0,
                egui::Color32::from_rgb(90, 170, 90),
                egui::Stroke::NONE,
            );
            // Two strokes form the check; egui 0.29's Painter has `segment`.
            p.line_segment(
                [c + egui::vec2(-3.6, 0.4), c + egui::vec2(-1.2, 3.2)],
                egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
            );
            p.line_segment(
                [c + egui::vec2(-1.2, 3.2), c + egui::vec2(4.0, -3.2)],
                egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
            );
        }
        "warn" => {
            p.circle(
                c,
                6.0,
                egui::Color32::from_rgb(210, 170, 80),
                egui::Stroke::NONE,
            );
            p.line_segment(
                [c + egui::vec2(0.0, -3.5), c + egui::vec2(0.0, 1.5)],
                egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
            );
            p.circle_filled(c + egui::vec2(0.0, 3.4), 1.4, egui::Color32::WHITE);
        }
        "wait" => {
            p.circle(
                c,
                6.0,
                egui::Color32::from_rgb(120, 120, 120),
                egui::Stroke::NONE,
            );
        }
        _ => {
            p.circle(
                c,
                6.0,
                egui::Color32::from_rgb(70, 70, 70),
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(150, 150, 150)),
            );
        }
    }
}

/// A store download stores launch_exe relative to the install folder; a
/// rescan stores it absolute. Spawning the relative name from the launcher's
/// own cwd fails with os error 2, so join it back onto the install dir.
fn resolve_launch_path(install_dir: &str, launch_exe: &str) -> PathBuf {
    let exe = PathBuf::from(launch_exe);
    if !exe.is_absolute() && !install_dir.is_empty() {
        return Path::new(install_dir).join(&exe);
    }
    exe
}

/// Folder a MelonLoader install should land in: the game at `idx`, only if
/// it is installed. Kept as a pure fn so the wrong-index bug is testable:
/// resolving via `selected` instead of the popup's idx installs into another
/// game while the popup names the first one.
/// Record what a launched game prints, so "nothing in the logs" cannot be
/// blamed on the launcher. Unity itself writes its own Player.log inside the
/// prefix (drive_c/users/.../AppData/LocalLow/<company>/<game>/Player.log on
/// Windows paths, ~/.config/unity3d/... on native Linux); this covers the
/// loader/runtime chatter that Unity never logs.
/// The process exit number, where the platform exposes one.
fn exit_of(st: &std::process::ExitStatus) -> Option<i32> {
    #[cfg(unix)]
    {
        st.code()
    }
    #[cfg(not(unix))]
    {
        if st.success() {
            Some(0)
        } else {
            None
        }
    }
}

fn capture_output(
    child: std::process::Child,
    label: String,
    hint_dir: PathBuf,
    gfx_note: Slot<String>,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut child = child;
    // Unity says "InitializeEngineGraphics failed" when D3D11 device creation
    // refused; the UI turns that into actionable advice instead of silence.
    let gfx_hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Proton says "Prefix has an invalid version?!" when a prefix was made by
    // a different Proton build; the game then dies right after the splash.
    let prefix_hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let out = child.stdout.take();
    let err = child.stderr.take();
    let mut drains: Vec<std::thread::JoinHandle<()>> = Vec::new();
    // Drain both pipes at once: draining stdout to EOF before touching stderr
    // deadlocks a game that writes >64 KB to stderr while stdout stays open —
    // the game hangs forever and nothing is ever logged. Lines land in the log
    // as they arrive, so a crash mid-run still leaves its tail behind.
    if let Some(o) = out {
        let tag = label.clone();
        let hit = gfx_hit.clone();
        let phit = prefix_hit.clone();
        drains.push(std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(o);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if !line.trim().is_empty() {
                    auth::log_line(&format!("[{tag}] {line}"));
                }
                if line.contains("InitializeEngineGraphics")
                    || line.contains("Failed to initialize graphics")
                {
                    hit.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                if line.contains("Prefix has an invalid version") {
                    phit.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }));
    }
    if let Some(e) = err {
        let tag = label.clone();
        let hit = gfx_hit.clone();
        let phit = prefix_hit.clone();
        drains.push(std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(e);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if !line.trim().is_empty() {
                    auth::log_line(&format!("[{tag}] {line}"));
                }
                if line.contains("InitializeEngineGraphics")
                    || line.contains("Failed to initialize graphics")
                {
                    hit.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                if line.contains("Prefix has an invalid version") {
                    phit.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }));
    }
    // Reap the child so repeated launches do not pile up zombies while the
    // window is open; the exit code is the first thing a silent crash needs.
    let tag = label;
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|st| exit_of(&st));
        running.store(false, std::sync::atomic::Ordering::Relaxed);
        // Join the pipe threads before judging the flags: the child exits
        // instantly in tests and short crashes, and its last lines can still
        // be in a drain thread's buffer when wait returns. Judging early made
        // the graphics/prefix advice land as silence.
        for d in drains {
            let _ = d.join();
        }
        if prefix_hit.load(std::sync::atomic::Ordering::Relaxed) {
            *gfx_note.lock().unwrap() = Some(Ok("prefix".into()));
        } else if gfx_hit.load(std::sync::atomic::Ordering::Relaxed) {
            *gfx_note.lock().unwrap() = Some(Ok("graphics".into()));
        }
        auth::log_line(&format!(
            "[{tag}] the game process ended{}; the game writes its own log under {} (look for Player.log there)",
            match code {
                Some(c) => format!(" with code {c}"),
                None => String::new(),
            },
            hint_dir.display()
        ));
    });
}

/// MelonLoader keeps Mods, Plugins, UserData and UserLibs next to version.dll.
/// The loader creates them when a game boots, but a game that dies during
/// graphics init never gets there — so the installer makes them up front.
fn melon_dirs(dest: &Path) -> usize {
    let mut made = 0usize;
    for name in ["Mods", "Plugins", "UserData", "UserLibs"] {
        if std::fs::create_dir_all(dest.join(name)).is_ok() {
            made += 1;
        }
    }
    made
}

fn melon_dest(apps: &[GameEntry], idx: usize) -> Option<PathBuf> {
    apps.get(idx)
        .filter(|g| !g.install_dir.is_empty())
        .map(|g| PathBuf::from(g.install_dir.clone()))
}

/// Status-bar severity (measured lesson: guidance rendered green reads as a
/// success that never happened). Substring classifier kept as a pure fn so
/// it can be unit-tested as a pure function.
fn status_tone(text: &str) -> &'static str {
    const GUIDANCE: [&str; 24] = [
        "cancelled",
        "cancel requested",
        "login started",
        "cleared",
        "waiting",
        "reported",
        "measuring",
        "nothing installed",
        "loading",
        "refreshing",
        "checking",
        "applying",
        "downloading",
        "installing",
        "already",
        "pick",
        "first",
        "not signed in",
        "older downloads",
        "wait for it",
        "install again",
        "not installed",
        "no Steam",
        "no longer",
    ];
    const ERROR: [&str; 13] = [
        "failed",
        "mismatch",
        "could not",
        "corrupt",
        "rejected",
        "error",
        "needs token",
        "no game",
        "no store version",
        "not verified",
        "missing",
        "not for this login",
        "did not accept",
    ];
    if ERROR.iter().any(|k| text.contains(k)) {
        "error"
    } else if GUIDANCE.iter().any(|k| text.contains(k)) {
        "guidance"
    } else {
        "success"
    }
}

/// Folder name for a game inside the download dir: lowercase words joined
/// by single dashes ("Bonelab VR" -> "bonelab-vr").
/// First playable Windows exe under a folder (three levels deep), preferring
/// the Unity-style *_Windows64.exe and skipping installer/setup binaries.
fn find_game_exe(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    let mut exe: Option<PathBuf> = None;
    while let Some((d, depth)) = stack.pop() {
        if depth > 3 {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if name.ends_with(".exe")
                && !name.contains("crashhandler")
                && !name.contains("setup")
                && !name.contains("installer")
            {
                if name.ends_with("_windows64.exe") {
                    return Some(path);
                }
                if exe.is_none() {
                    exe = Some(path);
                }
            }
        }
    }
    exe
}

/// Total bytes of a directory tree, walked iteratively.
fn dir_bytes(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(md) = e.metadata() {
                total += md.len();
            }
        }
    }
    total
}

fn human_size(bytes: u64) -> String {
    human_bytes(bytes)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whole days since the last launch; None when never played.
fn played_days(last: u64, now: u64) -> Option<u64> {
    if last == 0 {
        return None;
    }
    Some(now.saturating_sub(last) / 86_400)
}

/// The vendor installer metadata a store install leaves in the game folder:
/// real app id, canonical name, launch file and version. An externally
/// copied folder carrying this can be matched to its library entry instead
/// of guessed.
fn oculus_manifest(dir: &Path) -> Option<(String, String, String, String)> {
    let raw = std::fs::read(dir.join(".oculus-manifest.json")).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let app = v["appId"].as_str().filter(|t| !t.is_empty())?.to_string();
    let name = v["canonicalName"].as_str().unwrap_or("").to_string();
    let launch = v["launchFile"].as_str().unwrap_or("").to_string();
    let version = v["version"].as_str().unwrap_or("").to_string();
    Some((app, name, launch, version))
}

/// Name-based fallback match: folder equals the numeric app id (old install
/// dirs) or the store download folder name.
fn auto_match_store(apps: &[GameEntry], dir: &Path) -> Option<usize> {
    let base = dir.file_name()?.to_str()?.to_lowercase();
    apps.iter().position(|g| {
        !g.meta_app_id.is_empty()
            && (g.meta_app_id.eq_ignore_ascii_case(&base)
                || g.app_id.eq_ignore_ascii_case(&base)
                || folder_name(&g.name).eq_ignore_ascii_case(&base))
    })
}

fn folder_name(name: &str) -> String {
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

/// Long absolute paths look ugly in dropdowns. Keep the drive/root letter and
/// the last two components: /home/user/.local/steam/steamapps/common becomes
/// /h.../steamapps/common. Pure so it is unit-testable.
/// Path string for equality comparisons: Windows displays backslashes while
/// joined-arg strings may carry slashes, so normalize before comparing.
fn norm_path(s: &str) -> String {
    s.replace('\\', "/").to_lowercase()
}

fn short_path(p: &str) -> String {
    // Windows paths use '\', not MAIN_SEPARATOR-aware splitting: splitting a
    // Windows path there turned "C:\Steam\..." into "\C.../steamapps/common"
    // and failed the Windows build's own test. Split on '/' only.
    let parts: Vec<&str> = p.split('/').filter(|x| !x.is_empty()).collect();
    if parts.len() <= 3 {
        return p.to_string();
    }
    let first = parts[0].chars().next().unwrap_or('?');
    let last = parts[parts.len() - 1];
    let mid = parts[parts.len() - 2];
    format!("/{first}.../{mid}/{last}")
}

fn human_bytes(n: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = MIB * 1024;
    if n >= GIB {
        format!("{:.1} GiB", n as f64 / GIB as f64)
    } else if n >= MIB {
        format!("{:.0} MiB", n as f64 / MIB as f64)
    } else {
        format!("{n} B")
    }
}

fn pick_exe(dir: &Path) -> Option<String> {
    let mut best: Option<(u64, String)> = None;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let n = e.file_name().to_string_lossy().to_lowercase();
            if !n.ends_with(".exe") || n.contains("crashhandler") || n.contains("setup") {
                continue;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            let rel = p.strip_prefix(dir).unwrap_or(&p).display().to_string();
            if best.as_ref().map(|b| size > b.0).unwrap_or(true) {
                best = Some((size, rel));
            }
        }
    }
    best.map(|(_, rel)| rel)
}

impl App {
    fn data_root(&self) -> &Path {
        self.store_path.parent().unwrap_or(Path::new("."))
    }

    fn new(log_reset: bool) -> Self {
        let dat_dir = data_dir();
        std::fs::create_dir_all(&dat_dir).ok();
        std::fs::create_dir_all(dat_dir.join("prefixes")).ok();
        std::fs::create_dir_all(dat_dir.join("covers")).ok();
        auth::init_log(
            dat_dir.join("launcher.log").to_string_lossy().into_owned(),
            log_reset,
        );
        auth::log_line("=== launcher session start ===");
        let config_path = dat_dir.join("config.json");
        let store_path = dat_dir.join("games.json");

        let config_raw = std::fs::read(&config_path).ok();
        if config_raw.is_some()
            && config_raw
                .as_ref()
                .map(|b| serde_json::from_slice::<Config>(b).is_err())
                .unwrap_or(false)
        {
            auth::log_line("config.json could not be parsed - using defaults");
        }
        let fresh_config = config_raw.is_none();
        let mut config = config_raw
            .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
            .unwrap_or_default();
        if config.prefix_dir.is_empty() {
            // The prefix ROOT: every game gets its own prefix inside it, and
            // rescan walks the game prefixes there. A "shared" default made
            // the scan root dead because launches never wrote into it.
            config.prefix_dir = dat_dir.join("prefixes").to_string_lossy().to_string();
        }
        // On a machine with no config yet, match the desktop: its language
        // and its dark/light preference. A config that exists but parses
        // badly still gets the shipped defaults instead of re-detecting.
        if fresh_config {
            config.theme = system_theme();
            config.lang = system_lang();
            auth::log_line(&format!(
                "first run - UI matched the system: language {}, theme {}",
                config.lang, config.theme
            ));
        } else {
            // A Config read from disk can still carry empty theme or lang
            // from an older version; the dropdown labels need real values.
            if config.theme.is_empty() {
                config.theme = default_theme();
            }
            if config.lang.is_empty() {
                config.lang = default_lang();
            }
        }
        if config.download_dir.is_empty() {
            config.download_dir = dat_dir.join("games").to_string_lossy().to_string();
        }
        #[cfg(not(target_os = "windows"))]
        if config.proton_dirs.is_empty() {
            let home = std::env::var("HOME").unwrap_or_default();
            for c in [
                format!("{home}/.steam/steam/compatibilitytools.d"),
                format!("{home}/.steam/steam/steamapps/common"),
            ] {
                if Path::new(&c).is_dir() {
                    config.proton_dirs.push(c);
                }
            }
        }

        let store_raw = std::fs::read(&store_path).ok();
        if store_raw.is_some()
            && store_raw
                .as_ref()
                .map(|b| serde_json::from_slice::<GameStore>(b).is_err())
                .unwrap_or(false)
        {
            auth::log_line("games.json could not be parsed - library starts empty");
        }
        let store = store_raw
            .and_then(|b| serde_json::from_slice::<GameStore>(&b).ok())
            .unwrap_or_default();

        let session_path = dat_dir.join("session.json");
        let session_raw = std::fs::read(&session_path).ok();
        if session_raw.is_some()
            && session_raw
                .as_ref()
                .map(|b| serde_json::from_slice::<Session>(b).is_err())
                .unwrap_or(false)
        {
            auth::log_line("session.json could not be parsed - login cleared");
        }
        let session = session_raw
            .clone()
            .and_then(|b| serde_json::from_slice::<Session>(&b).ok())
            .unwrap_or_default();

        Self {
            config,
            config_path,
            store_path,
            session_path,
            session_bytes: session_raw.unwrap_or_default(),
            session,
            store,
            selected: 0,
            open_game: None,
            uninstall_ask: None,
            status: String::new(),
            settings_page: false,
            callback_url: String::new(),
            selected_branch: 0,
            redeem_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            redeem_inflight: false,
            fetch_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            fetch_inflight: false,
            fetch_token: String::new(),
            ctx: None,
            probe_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            probe_inflight: false,
            probe_token: String::new(),
            verified_account: None,
            install_prompt: None,
            melon_popup: None,
            #[cfg(not(target_os = "windows"))]
            proton_popup: false,
            token_revealed: false,
            size_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            disk_sizes: std::collections::HashMap::new(),
            disk_pending: None,
            disk_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            import_ask: None,
            win_swap_landed: None,
            size_inflight: false,
            size_pending_gen: 0,
            dl_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            dl_inflight: false,
            dl_progress: std::sync::Arc::new(std::sync::Mutex::new((0, 0, 0, 0))),
            dl_stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            running_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            running_pid: None,
            running_prefix: None,
            running_exe: None,
            stop_ask: None,
            melon_pending_idx: None,
            #[cfg(not(target_os = "windows"))]
            game_proton_popup: None,
            dl_version: String::new(),
            dl_version_code: 0,
            dl_binary_id: String::new(),
            token_snapshot: String::new(),
            token_changed_at: None,
            tex_cache: std::collections::HashMap::new(),
            covers_off: false,
            logo_tex: None,
            logo_aspect: 1.0,
            dots_tex: None,
            dir_popup: None,
            custom_pick: None,
            rel_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            rel_repo: String::new(),
            pending_repo: Vec::new(),
            rel_inflight: false,
            proton_releases: Vec::new(),
            melon_releases: Vec::new(),
            #[cfg(not(target_os = "windows"))]
            proton_pick: 0,
            melon_pick: 0,
            #[cfg(not(target_os = "windows"))]
            proton_to_steam: true,
            install_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            install_inflight: false,
            ch_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),

            ch_key: String::new(),
            ch_inflight: false,
            chk_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            chk_inflight: false,
            dl_key: String::new(),
            dl_label: String::new(),
            launch_slot: std::sync::Arc::new(std::sync::Mutex::new(None)),
            tweak_popup: None,
            tweak_args: String::new(),
            tweak_env: String::new(),
        }
    }

    /// Fill the library as soon as a bearer shows up (loaded from
    /// session.json, minted by redeem, or pasted in settings) — exactly once
    /// per token, never on every frame. A failed fetch is not retried
    /// automatically; the refresh button covers that.
    fn auto_fetch_owned(&mut self) {
        if self.session.token.is_empty() || self.fetch_token == self.session.token {
            return;
        }
        // A typed or pasted token changes on every keystroke. Wait until it
        // settles for ~1s, otherwise half-typed tokens burn the fetch slot and
        // the finished token never gets fetched.
        let settled = self
            .token_changed_at
            .map(|t| t.elapsed() >= std::time::Duration::from_millis(900))
            .unwrap_or(true);
        if !settled {
            return;
        }
        auth::log_line("bearer present — fetching owned apps automatically");
        let probe_will_run = !self.probe_inflight && self.probe_token != self.session.token;
        let fetch_will_run = !self.fetch_inflight;
        self.start_probe();
        self.fetch_owned();
        // Mark this token handled only if the workers actually started; a
        // blocked fetch retries with the final token on a later frame.
        if fetch_will_run && (probe_will_run || self.probe_token == self.session.token) {
            self.fetch_token = self.session.token.clone();
        }
    }

    /// Ask the store who this bearer belongs to. The "signed in" marker is
    /// only green when this probe comes back 200 — a pasted token that the
    /// API rejects must not pretend to be a login.
    fn start_probe(&mut self) {
        if self.session.token.is_empty() || self.probe_token == self.session.token {
            return;
        }
        if self.probe_inflight {
            return;
        }
        self.probe_token = self.session.token.clone();
        self.verified_account = None;
        self.probe_inflight = true;
        // Drop any leftover result from a probe whose inflight flag was
        // cleared (e.g. Clear session), so the next poll cannot mistake the
        // old token's verdict for the new token's.
        *self.probe_slot.lock().unwrap() = None;
        let token = self.session.token.clone();
        let slot = self.probe_slot.clone();
        let repaint = self.ctx.clone();
        std::thread::spawn(move || {
            let res = fetcher::account_id(&token);
            *slot.lock().unwrap() = Some((token, res));
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    /// Merge the account probe result.
    fn poll_probe(&mut self) {
        if !self.probe_inflight {
            return;
        }
        let Some((tok, res)) = self.probe_slot.lock().unwrap().take() else {
            return;
        };
        if tok != self.probe_token || tok != self.session.token {
            // A probe of a bearer the user already replaced: never certify the
            // new token with the old account's verdict. If the bearer was
            // swapped mid-probe, restart the probe for the current token.
            if tok != self.session.token && self.session.token != self.probe_token {
                self.probe_inflight = false;
                self.start_probe();
            }
            return;
        }
        self.probe_inflight = false;
        match res {
            Ok(id) => {
                self.verified_account = Some(id);
                auth::log_line(&format!("account probe OK: {id}"));
                self.status = "login confirmed — the token still works".into();
            }
            Err(e) => {
                self.verified_account = None;
                auth::log_line(&format!("account probe failed: {e}"));
                self.status = format!("token rejected by the store: {e}");
            }
        }
    }

    /// A callback from the protocol handler lands in session.json via a
    /// separate short-lived process, so the GUI must notice the file change.
    /// mtime alone is too coarse (one-second granularity can hide a write in
    /// the same second), so compare the actual loaded fields every tick.
    fn refresh_session(&mut self) {
        let was_signed = !self.session.token.is_empty();
        if !apply_disk_session(
            &mut self.session,
            &mut self.session_bytes,
            &self.session_path,
        ) {
            return;
        }
        if !was_signed && !self.session.token.is_empty() {
            auth::log_line("bearer loaded from session.json — logged in");
            self.status = "logged in - token saved".into();
        }
        if !self.session.frl_request.is_empty()
            && !self.session.frl_blob.is_empty()
            && !self.session.callback_attempted
            && !self.redeem_inflight
        {
            auth::log_line(&format!(
                "callback arrived token={} blob={}",
                auth::log_peek(&self.session.frl_request),
                self.session.frl_blob
            ));
            // The blob is single-use and the vendor expires it server-side, so
            // redeem the moment the callback lands instead of waiting for
            // a button press. Only once per callback: a failed attempt
            // marks callback_attempted so this cannot loop forever.
            self.start_redeem();
        }
    }

    fn save_session(&mut self) {
        if let Ok(s) = serde_json::to_string(&self.session) {
            std::fs::write(&self.session_path, &s).ok();
            // Remember what we just wrote, so refresh_session does not treat
            // our own write as an external callback and reload it.
            self.session_bytes = s.into_bytes();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(
                    &self.session_path,
                    std::fs::Permissions::from_mode(0o600),
                )
                .ok();
            }
        }
    }

    fn login_in_browser(&mut self) {
        match auth::start_login() {
            Ok((url, sso_token, etoken)) => {
                // Any callback stored before this press belongs to a rotated, dead
                // token pair; drop it so the GUI can't offer a stale redeem.
                self.session.frl_request.clear();
                self.session.frl_blob.clear();
                self.session.native_sso_manual.clear();
                self.session.callback_attempted = false;
                self.callback_url.clear();
                self.session.native_sso_token = sso_token;
                self.session.native_sso_etoken = etoken;
                auth::log_line(&format!(
                    "login started sso={} etoken={}",
                    auth::log_peek(&self.session.native_sso_token),
                    auth::log_peek(&self.session.native_sso_etoken)
                ));
                self.save_session();
                match auth::open_browser(&url) {
                    Ok(()) => self.status = "login started - finish in the browser, then press Apply when the browser sends back".to_string(),
                    Err(e) => self.status = format!("{e}\nURL: {url}"),
                }
            }
            Err(e) => self.status = format!("login start failed: {e}"),
        }
    }

    fn redeem_callback(&mut self) {
        if !self.callback_url.is_empty() {
            match auth::callback_params(&self.callback_url) {
                Some((t, b)) => {
                    auth::log_line(&format!("callback pasted token={t} blob={b}"));
                    self.session.frl_request = t;
                    self.session.frl_blob = b;
                    self.callback_url.clear();
                    // Persist even when start_redeem below refuses (no token
                    // yet, redeem in flight): without this the pasted link is
                    // lost on close and a handler write reloads old params.
                    self.save_session();
                }
                None => {
                    self.status = "the login link is missing its login token and login code".into();
                    return;
                }
            }
        }
        // Manual button may retry even after a failed attempt.
        self.session.callback_attempted = false;
        self.start_redeem();
    }

    /// Gate the callback against the stored native SSO token pair, then run
    /// the exchange on a background thread. The retry rounds block for seconds
    /// and must never run on the UI thread.
    fn start_redeem(&mut self) {
        if self.redeem_inflight {
            self.status = "login token already being applied".into();
            return;
        }
        if self.session.frl_request.is_empty() || self.session.frl_blob.is_empty() {
            self.status = "paste the browser login link, or press Login first".into();
            return;
        }
        if self.session.native_sso_token.is_empty() && self.session.native_sso_manual.is_empty() {
            self.status =
                "no login token here — press Login in the browser first; this app only works with the login it started itself".into();
            return;
        }
        // Pick the token whose sha256[..16] equals the callback token param.
        let mut request_token: Option<String> = None;
        if !self.session.native_sso_token.is_empty()
            && auth::callback_token_matches(
                &self.session.native_sso_token,
                &self.session.frl_request,
            )
        {
            request_token = Some(self.session.native_sso_token.clone());
        }
        if request_token.is_none()
            && !self.session.native_sso_manual.is_empty()
            && auth::callback_token_matches(
                &self.session.native_sso_manual,
                &self.session.frl_request,
            )
        {
            request_token = Some(self.session.native_sso_manual.clone());
            self.status = "using the pasted login token".into();
        }
        let Some(rt) = request_token else {
            self.status = "that link is not for this login - press Login once, then apply the link it opens right away".into();
            auth::log_line("hash mismatch: last resort is DevTools Network, filter tokens_query, copy native_sso_token into the fallback field");
            return;
        };
        // The vendor client sends native_sso_token as request_token, not the URL token param.
        let etoken = self.session.native_sso_etoken.clone();
        let blob = self.session.frl_blob.clone();
        let slot = self.redeem_result.clone();
        let repaint = self.ctx.clone();
        // Purge a leftover result from an aborted redeem, otherwise the
        // next Apply consumes an old callback's bearer and signs in wrongly.
        *slot.lock().unwrap() = None;
        self.redeem_inflight = true;
        self.status = "applying login token (up to ~7s)".into();
        std::thread::spawn(move || {
            let res = auth::exchange(&rt, &etoken, &blob);
            *slot.lock().unwrap() = Some(res);
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    /// Pick up the background redeem result.
    fn poll_redeem(&mut self) {
        if !self.redeem_inflight {
            return;
        }
        let done = self.redeem_result.lock().unwrap().take();
        let Some(res) = done else { return };
        self.redeem_inflight = false;
        match res {
            Ok(bearer) => {
                self.session.token = bearer;
                self.session.frl_request.clear();
                self.session.frl_blob.clear();
                self.session.native_sso_token.clear();
                self.session.callback_attempted = false;
                self.save_session();
                auth::log_line("bearer token minted and saved");
                self.status = "signed in — login applied".into();
            }
            Err(e) => {
                auth::log_line(&format!("redeem failed: {e}"));
                self.session.callback_attempted = true;
                self.save_session();
                self.status = format!(
                    "login failed: {e} - press Login once, then apply the link it opens right away"
                );
            }
        }
    }

    fn clear_session(&mut self) {
        self.session = Session::default();
        std::fs::remove_file(&self.session_path).ok();
        // With no file left, forget the remembered bytes: an empty cache lets
        // a later handler callback write reload normally.
        self.session_bytes = Vec::new();
        // Drop probe state too, otherwise the green marker and the once-per-
        // token auto-fetch survive a cleared bearer.
        self.verified_account = None;
        self.probe_token = String::new();
        self.probe_inflight = false;
        *self.probe_slot.lock().unwrap() = None;
        self.fetch_token = String::new();
        // Same rule as probe/redeem: a fetch in flight dies with the session,
        // otherwise fetch_inflight sticks true forever after Forget token and
        // the refresh button refuses every later fetch.
        self.fetch_inflight = false;
        *self.fetch_result.lock().unwrap() = None;
        // A redeem in flight must die with the session too: forgetting the
        // token while the exchange runs would otherwise resurrect a green
        // "signed in" from the landed bearer and stick redeem_inflight true.
        self.redeem_inflight = false;
        *self.redeem_result.lock().unwrap() = None;
        self.status = "session cleared".into();
    }

    /// Store binary for the selected channel of one entry ("" = no store version).
    fn binary_id_for(&self, idx: usize) -> String {
        match self.store.apps.get(idx) {
            Some(g) => match g.branches.get(self.selected_branch) {
                Some(b) => b.binary_id.clone(),
                None => g.binary_id.clone(),
            },
            None => String::new(),
        }
    }

    /// Whether Install can actually run for this entry right now. The button
    /// must mirror open_install's refusals (empty token, busy download, scan
    /// entry without a store version), otherwise clicking always errors.
    fn installable(&self, idx: usize) -> bool {
        !self.session.token.is_empty() && !self.dl_inflight && !self.binary_id_for(idx).is_empty()
    }

    /// Is THIS game the one currently downloading? Positional indices drift,
    /// so compare by app id.
    fn dl_busy(&self, idx: usize) -> bool {
        self.dl_inflight
            && !self.dl_key.is_empty()
            && self
                .store
                .apps
                .get(idx)
                .map(|e| e.app_id == self.dl_key)
                .unwrap_or(false)
    }

    /// Open the install popup: ask the CDN how big the bundle is BEFORE
    /// downloading, let the user pick the folder, then install on confirm.
    fn open_install(&mut self) {
        if self.session.token.is_empty() {
            self.status = "not signed in yet — log in first".into();
            return;
        }
        if self.dl_inflight {
            self.status = "a download is already running".into();
            return;
        }
        let Some(g) = self.store.apps.get(self.selected).cloned() else {
            self.status = "no game selected".into();
            return;
        };
        let branch = g.branches.get(self.selected_branch);
        let binary_id = self.binary_id_for(self.selected);
        if binary_id.is_empty() {
            self.status = "this entry has no store version yet — press refresh first".into();
            return;
        }
        let channel = branch
            .map(|b| format!("{} ({})", b.name, b.version))
            .unwrap_or_else(|| "latest".into());
        let token = self.session.token.clone();
        let slot = self.size_slot.clone();
        let repaint = self.ctx.clone();
        // Drop any answer from a superseded lookup before starting the new one.
        *slot.lock().unwrap() = None;
        let gen = self.size_pending_gen + 1;
        self.size_pending_gen = gen;
        self.size_inflight = true;
        self.install_prompt = Some(InstallPrompt {
            app_idx: self.selected,
            binary_id: binary_id.clone(),
            version: branch.map(|b| b.version.clone()).unwrap_or_default(),
            version_code: branch.map(|b| b.version_code).unwrap_or(g.version_code),
            channel,
            files: 0,
            bytes: 0,
            dir: if g.install_dir.is_empty() {
                // One folder per game inside the download dir, like the
                // official launcher does (games/boneworks), not everything
                // loose in the games root.
                Path::new(&self.config.download_dir)
                    .join(folder_name(&g.name))
                    .display()
                    .to_string()
            } else {
                g.install_dir.clone()
            },
            sizing: true,
        });
        std::thread::spawn(move || {
            let res = fetcher::bundle_size(&token, &binary_id);
            let mut s = slot.lock().unwrap();
            if size_write_allowed(s.as_ref().map(|(g, _)| *g), gen) {
                *s = Some((gen, res));
            }
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    /// Merge the size estimate into the popup once the manifest answers.
    fn poll_size(&mut self) {
        if !self.size_inflight {
            return;
        }
        let Some((gen, res)) = self.size_slot.lock().unwrap().take() else {
            return;
        };
        if gen != self.size_pending_gen {
            // Answer from a lookup the user already replaced; keep waiting
            // for the current one instead of painting the wrong size.
            return;
        }
        self.size_inflight = false;
        let Some(p) = self.install_prompt.as_mut() else {
            return;
        };
        p.sizing = false;
        match res {
            Ok((files, bytes)) => {
                p.files = files;
                p.bytes = bytes;
            }
            Err(e) => {
                self.install_prompt = None;
                self.status = format!("size lookup failed: {e}");
            }
        }
    }

    /// Start the actual download on a background thread (a multi-GB bundle
    /// must never block the UI) and keep the progress counter updated.
    fn confirm_install(&mut self) {
        let Some(p) = self.install_prompt.take() else {
            return;
        };
        if p.sizing {
            self.status = "still measuring the download size".into();
            return;
        }
        let token = self.session.token.clone();
        let dest = PathBuf::from(p.dir.trim());
        let binary_id = p.binary_id.clone();
        self.dl_version = p.version.clone();
        self.dl_version_code = p.version_code;
        self.dl_binary_id = binary_id.clone();
        self.dl_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // Fresh numbers: the bar must not start at the previous download's 100%.
        *self.dl_progress.lock().unwrap() = (0, 0, 0, 0);
        let stop = self.dl_stop.clone();
        let prog = self.dl_progress.clone();
        let slot = self.dl_slot.clone();
        let repaint = self.ctx.clone();
        self.dl_inflight = true;
        self.dl_key = self
            .store
            .apps
            .get(p.app_idx)
            .map(|e| e.app_id.clone())
            .unwrap_or_default();
        self.status = format!(
            "downloading {} ({} files, {}) to {}",
            self.store
                .apps
                .get(p.app_idx)
                .map(|g| g.name.clone())
                .unwrap_or_default(),
            p.files,
            human_bytes(p.bytes),
            dest.display()
        );
        auth::log_line(&format!(
            "install {} binary {binary_id} into {}",
            self.store
                .apps
                .get(p.app_idx)
                .map(|g| g.app_id.clone())
                .unwrap_or_default(),
            dest.display()
        ));
        std::thread::spawn(move || {
            let res = fetcher::download_binary(
                &token,
                &binary_id,
                &dest,
                &stop,
                &|done, total, bytes, tot_bytes| {
                    *prog.lock().unwrap() = (done, total, bytes, tot_bytes);
                },
            );
            *slot.lock().unwrap() = Some(res.map(|_| dest));
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    /// Register the downloaded folder and report integrity.
    fn poll_download(&mut self) {
        if !self.dl_inflight {
            return;
        }
        let Some(res) = self.dl_slot.lock().unwrap().take() else {
            return;
        };
        self.dl_inflight = false;
        match res {
            Ok(dest) => {
                if let Some(exe) = pick_exe(&dest) {
                    // A Rescan during this download pushed a local twin of the
                    // same folder (the store entry had no install_dir yet).
                    // Drop twins pointing at this folder so one game = one card.
                    let dest_text = dest.display().to_string();
                    let before = self.store.apps.len();
                    self.store.apps.retain(|e| {
                        !(e.meta_app_id.is_empty()
                            && e.app_id != self.dl_key
                            && e.install_dir == dest_text)
                    });
                    if self.store.apps.len() != before {
                        // Positions shifted: any open popup keyed by index
                        // would name one game and act on another.
                        self.uninstall_ask = None;
                        self.melon_popup = None;
                        self.install_prompt = None;
                        self.open_game = None;
                        self.tweak_popup = None;
                        #[cfg(not(target_os = "windows"))]
                        {
                            self.game_proton_popup = None;
                        }
                        self.custom_pick = None;
                        self.selected = 0;
                        self.selected_branch = 0;
                    }
                    // Resolve BY APP ID after the twin purge: the retain above
                    // shifts positions, so a positional index could stamp a
                    // different game. Metadata and partial reset happen before
                    // the single save, so a finished install never persists
                    // partial:true.
                    if let Some(e) = self.store.apps.iter_mut().find(|e| e.app_id == self.dl_key) {
                        e.install_dir = dest.display().to_string();
                        e.launch_exe = exe;
                        if !self.dl_binary_id.is_empty() {
                            e.version = self.dl_version.clone();
                            e.version_code = self.dl_version_code;
                            e.binary_id = self.dl_binary_id.clone();
                        }
                        e.partial = false;
                    }
                    self.save_store();
                    self.status = format!("installed into {}", dest.display());
                } else {
                    self.status = format!(
                        "download failed: finished in {} but no exe found — press Rescan folders",
                        dest.display()
                    );
                }
                self.install_prompt = None;
            }
            Err(e) => {
                // Snapshot the key BEFORE clearing it: clearing first made the
                // partial lookup below find nothing, so a cancel never marked
                // the game partial and Rescan listed half folders as playable.
                let key = std::mem::take(&mut self.dl_key);
                if e.contains("cancelled") {
                    let (done, _, _, _) = *self.dl_progress.lock().unwrap();
                    // A cancelled FIRST install leaves a half-written folder
                    // that Rescan would list as a playable corrupt game. Only
                    // delete it when the folder was empty before this install;
                    // a reinstall into an existing folder must keep its files.
                    // Cancel keeps what was written (the next install reuses
                    // intact files) and marks the game partial, so the library
                    // never shows an unfinished game as installed.
                    let at = self.store.apps.iter().position(|e| e.app_id == key);
                    if let Some(i) = at {
                        self.store.apps[i].partial = true;
                        let name = self.store.apps[i].name.clone();
                        self.save_store();
                        self.status = format!(
                            "download cancelled after {done} files — {name} marked as partial, files kept for the next install"
                        );
                    } else {
                        self.status = format!("download cancelled after {done} files");
                    }
                } else {
                    // A CDN/checksum failure leaves a half folder too: mark it
                    // partial like Cancel and the CLI do, or Rescan would list
                    // a corrupt folder as a playable game.
                    let at = self.store.apps.iter().position(|e| e.app_id == key);
                    if let Some(i) = at {
                        self.store.apps[i].partial = true;
                        let name = self.store.apps[i].name.clone();
                        self.save_store();
                        self.status = format!("download failed: {e} — {name} marked as partial, files kept for the next install");
                    } else {
                        self.status = format!("download failed: {e}");
                    }
                }
            }
        }
    }

    /// GitHub version picker: Proton GE and MelonLoader release lists.
    fn load_releases(&mut self, repo: &str) {
        if self.rel_inflight {
            // Remember the request instead of dropping it: while the Melon
            // list loads, pressing "Choose a Proton version" used to sit on
            // "loading the release list…" forever, because nothing retried.
            // Queue every repo asked for, not just the newest press: a second
            // press of Proton used to erase MelonLoader's queued request and
            // its popup hung on "loading…" forever.
            if !self.pending_repo.iter().any(|r| r == repo) {
                self.pending_repo.push(repo.to_string());
            }
            self.status = "a version list is already loading".into();
            return;
        }
        self.rel_repo = repo.to_string();
        self.rel_inflight = true;
        self.status = "loading versions from GitHub…".into();
        let repo = repo.to_string();
        let slot = self.rel_slot.clone();
        let repaint = self.ctx.clone();
        // Purge any leftover result so a fresh request never consumes the
        // previous repo's release list.
        *slot.lock().unwrap() = None;
        std::thread::spawn(move || {
            // MelonLoader games here are Windows builds, so take the Windows
            // zip; Proton GE is a Linux tool and must skip win/mac archives.
            let avoid: &[&str] = if repo == "LavaGang/MelonLoader" {
                &["linux", "mac", "macos"]
            } else {
                // GE-Proton ships per-arch tarballs; aarch64 is the wrong
                // build for this machine and must never be offered.
                &["win", "mac", "aarch64", "arm64", "arm"]
            };
            let res = fetcher::github_releases(&repo, avoid);
            *slot.lock().unwrap() = Some(res);
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    fn poll_releases(&mut self) {
        if !self.rel_inflight {
            return;
        }
        let Some(res) = self.rel_slot.lock().unwrap().take() else {
            return;
        };
        self.rel_inflight = false;
        let retry = std::mem::take(&mut self.pending_repo);
        match res {
            Ok(list) => {
                if self.rel_repo == "GloriousEggroll/proton-ge-custom" {
                    self.proton_releases = list;
                } else {
                    self.melon_releases = list;
                }
                self.status = format!("{} versions listed", self.rel_repo);
            }
            Err(e) => {
                self.status = format!("version list failed: {e}");
                // A failed load must still fire a queued request, even for the
                // SAME repo pressed twice — otherwise the popup hangs on
                // "loading the version list…" forever.
                for r in &retry {
                    self.load_releases(r);
                }
            }
        }
        // Fire every queued repo that is not the one just delivered; the
        // popup that asked while another was loading must not hang forever.
        for r in &retry {
            if r != &self.rel_repo {
                self.load_releases(r);
            }
        }
    }

    /// Unpack a chosen Proton GE build into Steam's compatibilitytools.d
    /// (default, picked from Steam) or the launcher's own proton dir.
    #[cfg(not(target_os = "windows"))]
    fn install_proton(&mut self) {
        let Some((tag, name, url)) = self.proton_releases.get(self.proton_pick).cloned() else {
            self.status = "list Proton versions and pick one first".into();
            return;
        };
        if self.install_inflight {
            self.status = "an installer is already running".into();
            return;
        }
        let dest = if self.proton_to_steam && steam_present() {
            steam_compat_dir()
        } else {
            if self.proton_to_steam {
                self.status = "no Steam here — installing into the launcher folder instead".into();
            }
            self.data_root().join("proton")
        };
        self.install_inflight = true;
        self.dl_inflight = true;
        self.dl_key.clear();
        self.dl_label = format!("Proton {tag}");
        self.dl_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        *self.dl_progress.lock().unwrap() = (0, 0, 0, 0);
        self.status = i18n::fill(
            i18n::text(i18n::parse(&self.config.lang), "install_tool_x").as_str(),
            &[("tag", &tag), ("size", "500 MB")],
        );
        let slot = self.install_slot.clone();
        let repaint = self.ctx.clone();
        let prog = self.dl_progress.clone();
        let stop = self.dl_stop.clone();
        std::thread::spawn(move || {
            let res = fetcher::install_release(&url, &dest, &prog, &stop);
            *slot.lock().unwrap() =
                Some(res.map(|_| format!("{name} unpacked into {}", dest.display())));
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    /// Install the chosen MelonLoader release into ONE game, addressed by
    /// index — the popup names that game, while self.selected can drift to
    /// another card if the user navigated to the library meanwhile.
    fn install_melon(&mut self, idx: usize) {
        let Some((tag, _name, url)) = self.melon_releases.get(self.melon_pick).cloned() else {
            self.status = "pick a MelonLoader version first".into();
            return;
        };
        let dest = match melon_dest(&self.store.apps, idx) {
            Some(d) => d,
            None => {
                self.status = "this game is not installed yet".into();
                return;
            }
        };
        if self.install_inflight {
            self.status = "an installer is already running".into();
            return;
        }
        if self.dl_busy(idx) {
            self.status = "this game is downloading right now — wait for it".into();
            return;
        }
        self.melon_pending_idx = Some(idx);
        self.install_inflight = true;
        self.dl_inflight = true;
        self.dl_key.clear();
        self.dl_label = format!("MelonLoader {tag}");
        self.dl_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        *self.dl_progress.lock().unwrap() = (0, 0, 0, 0);
        self.status = i18n::fill(
            i18n::text(i18n::parse(&self.config.lang), "install_tool_x").as_str(),
            &[("tag", &tag), ("size", "2 MB")],
        );
        let lang = self.config.lang.clone();
        let slot = self.install_slot.clone();
        let repaint = self.ctx.clone();
        let prog = self.dl_progress.clone();
        let stop = self.dl_stop.clone();
        std::thread::spawn(move || {
            let res = fetcher::install_release(&url, &dest, &prog, &stop);
            let dirs = if res.is_ok() { melon_dirs(&dest) } else { 0 };
            if dirs > 0 {
                auth::log_line("MelonLoader folders ready: Mods, Plugins, UserData, UserLibs");
            }
            *slot.lock().unwrap() = Some(res.map(|_| {
                i18n::fill(
                    &i18n::text(i18n::parse(&lang), "tool_installed"),
                    &[("tag", &tag), ("dir", &dest.display().to_string())],
                )
            }));
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    /// Download the ReviveNext installer (NSIS, needs admin when run) into the
    /// launcher's tools dir. The installer itself is what drops the Revive
    /// translation DLLs; the launcher never registers SteamVR apps.
    #[cfg(target_os = "windows")]
    fn download_revive_setup(&mut self) {
        if self.install_inflight {
            self.status = "an installer is already running".into();
            return;
        }
        let list = match fetcher::github_releases(
            "Konstruukt-Labs/ReviveNext",
            &["linux", "mac", "macos"],
        ) {
            Ok(l) => l,
            Err(e) => {
                self.status = format!("version list failed: {e}");
                return;
            }
        };
        let Some((tag, name, url)) = list.first().cloned() else {
            self.status = "no ReviveNext release found".into();
            return;
        };
        let dest = self.data_root().join("tools").join(name.clone());
        self.install_inflight = true;
        self.dl_inflight = true;
        self.dl_key.clear();
        self.dl_label = format!("ReviveNext {tag}");
        self.dl_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        *self.dl_progress.lock().unwrap() = (0, 0, 0, 0);
        self.status = format!("downloading {name} ({tag})");
        let slot = self.install_slot.clone();
        let repaint = self.ctx.clone();
        let prog = self.dl_progress.clone();
        let stop = self.dl_stop.clone();
        std::thread::spawn(move || {
            let res = fetcher::save_release_file(&url, &dest, &prog, &stop);
            *slot.lock().unwrap() =
                Some(res.map(|_| format!("{name} saved into {}", dest.display())));
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    /// Point the launch hook at whatever a Revive or ReviveNext install left
    /// on this machine, so games load the OpenXR translation layer without any
    /// further paths being typed.
    #[cfg(target_os = "windows")]
    /// Swap the vendor runtime files the Unity headset plugin resolves from its
    /// install folder: beside-exe copies never redirect it (its log names the
    /// D: runtime path), so the replacement must land where the plugin looks.
    /// Elevating via PowerShell asks UAC once; originals are backed up first.
    #[cfg(target_os = "windows")]
    fn take_meta_runtime(&mut self) {
        let Some(dir) = meta_runtime_dir() else {
            self.status = "no vendor runtime folder found on this machine".to_string();
            return;
        };
        let xr_dll = self.data_root().join("LibReviveXR64.dll");
        std::fs::write(&xr_dll, REVIVE_XR_BYTES).ok();
        let impl_path = dir.join("LibOVRRTImpl64_1.dll");
        if dll_matches(&impl_path, REVIVE_XR_BYTES.len() as u64) {
            self.status = "vendor runtime is already replaced".to_string();
            return;
        }
        let backup = self.data_root().join("iris-runtime-backup");
        std::fs::create_dir_all(&backup).ok();
        for n in ["LibOVRRT64_1.dll", "LibOVRRTImpl64_1.dll"] {
            let o = dir.join(n);
            let b = backup.join(n);
            if o.is_file() && !b.is_file() {
                std::fs::copy(&o, &b).ok();
            }
        }
        let xr = xr_dll.to_string_lossy().to_string();
        let rt_s = dir.join("LibOVRRT64_1.dll").to_string_lossy().to_string();
        let impl_s = impl_path.to_string_lossy().to_string();
        if meta_runtime_swap(&[(xr.as_str(), rt_s.as_str()), (xr.as_str(), impl_s.as_str())]) {
            self.status =
                "runtime swap ran elevated, originals saved in iris-runtime-backup".to_string();
        } else {
            self.status = "elevated swap script could not start".to_string();
        }
    }

    /// Backup folder for the runtime swap. New swaps write
    /// iris-runtime-backup; restore still finds the older
    /// meta-runtime-backup if that is what exists on disk.
    #[cfg(target_os = "windows")]
    fn runtime_backup_dir(&self) -> std::path::PathBuf {
        let fresh = self.data_root().join("iris-runtime-backup");
        if fresh.exists() {
            fresh
        } else {
            self.data_root().join("meta-runtime-backup")
        }
    }

    #[cfg(target_os = "windows")]
    fn restore_meta_runtime(&mut self) {
        let Some(dir) = meta_runtime_dir() else {
            self.status = "no vendor runtime folder found to restore".to_string();
            return;
        };
        let backup = self.runtime_backup_dir();
        let mut pairs: Vec<(String, String)> = Vec::new();
        for n in ["LibOVRRT64_1.dll", "LibOVRRTImpl64_1.dll"] {
            let b = backup.join(n);
            if b.is_file() {
                pairs.push((
                    b.to_string_lossy().to_string(),
                    dir.join(n).to_string_lossy().to_string(),
                ));
            }
        }
        if pairs.is_empty() {
            self.status = "nothing backed up to restore".to_string();
            return;
        }
        let refs: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        if meta_runtime_swap(&refs) {
            self.status = "original vendor runtime restored via the elevated script".to_string();
        } else {
            self.status = "restore script could not start".to_string();
        }
    }

    #[cfg(target_os = "windows")]
    fn find_revive_layer(&mut self) {
        let (dll, server) = revive_scan(&revive_scan_roots());
        match (dll.clone(), server.clone()) {
            (Some(d), Some(s)) => {
                self.config.revive_dll = d.to_string_lossy().into_owned();
                self.config.ovr_server = s.to_string_lossy().into_owned();
                self.save_config();
                self.status = format!(
                    "layer found: {} + runtime {}",
                    short_path(&d.to_string_lossy()),
                    short_path(&s.to_string_lossy())
                );
            }
            (Some(d), None) => {
                self.config.revive_dll = d.to_string_lossy().into_owned();
                self.save_config();
                self.status = format!(
                    "layer found: {} - no runtime peer, games boot without entitlement checks",
                    short_path(&d.to_string_lossy())
                );
            }
            (None, Some(s)) => {
                self.config.ovr_server = s.to_string_lossy().into_owned();
                self.save_config();
                self.status = format!(
                    "runtime peer found: {} - install the Revive layer for it to translate",
                    short_path(&s.to_string_lossy())
                );
            }
            (None, None) => self.status = "no Revive layer found in the install folders".into(),
        }
    }

    /// Launch the downloaded NSIS installer so Windows shows its own UAC card.
    #[cfg(target_os = "windows")]
    fn run_revive_installer(&mut self) {
        let setup = self.data_root().join("tools").join("ReviveNext-Setup.exe");
        if !setup.is_file() {
            self.status = "download the ReviveNext installer first".into();
            return;
        }
        // NSIS setups declare a required elevation level, plain spawn refuses
        // with os error 740. PowerShell's Start-Process -Verb RunAs shows the
        // real UAC card instead.
        let path = setup.to_string_lossy().into_owned();
        let ps = format!(
            "Start-Process -Verb RunAs -FilePath '{}'",
            path.replace("'", "''")
        );
        match std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .spawn()
        {
            Ok(_) => self.status = "installer started with elevation - approve the UAC card".into(),
            Err(e) => self.status = format!("installer could not start: {e}"),
        }
    }

    fn poll_install(&mut self) {
        if !self.install_inflight {
            return;
        }
        let Some(res) = self.install_slot.lock().unwrap().take() else {
            return;
        };
        self.install_inflight = false;
        self.dl_inflight = false;
        self.dl_label.clear();
        match res {
            Ok(msg) => {
                self.status = msg.clone();
                auth::log_line(&msg);
                // A fresh Proton dir should be visible to the picker right away.
                #[cfg(not(target_os = "windows"))]
                if msg.contains("proton") || msg.contains("compatibilitytools") {
                    let dir = msg.split(" into ").nth(1).unwrap_or("").to_string();
                    if !dir.is_empty() && !self.config.proton_dirs.contains(&dir) {
                        self.config.proton_dirs.push(dir);
                        if let Ok(s) = serde_json::to_string_pretty(&self.config) {
                            std::fs::write(&self.config_path, s).ok();
                        }
                    }
                }
            }
            Err(e) => {
                self.status = format!("installer failed: {e}");
                self.melon_pending_idx = None;
            }
        }
        // A finished MelonLoader install is stored on the game, not hidden in
        // the launch code: the option shows in Launch options and its radio
        // can turn it off without deleting the loader.
        if let Some(idx) = self.melon_pending_idx.take() {
            #[cfg(target_os = "windows")]
            let _ = idx;
            #[cfg(not(target_os = "windows"))]
            if let Some(e) = self.store.apps.get_mut(idx) {
                let mut env = env_lines(&e.launch_env.join("\n"));
                if toggle_dll_override(&mut env, "version=n,b", true) {
                    e.launch_env = env;
                    auth::log_line(
                        "MelonLoader dll override added to launch options (version=n,b)",
                    );
                    self.save_store();
                }
            }
        }
    }

    /// Plain synchronous download for the headless `--download` CLI.
    fn download_cli(&mut self, meta_id: &str, dest_override: Option<&str>) -> String {
        if self.session.token.is_empty() {
            return "not signed in yet — log in first".into();
        }
        let idx = match self
            .store
            .apps
            .iter()
            .position(|g| g.meta_app_id == meta_id || g.app_id == meta_id)
        {
            Some(i) => i,
            None => return format!("app {meta_id} is not in the library — fetch owned apps first"),
        };
        let binary_id = self.store.apps[idx].binary_id.clone();
        if binary_id.is_empty() {
            return format!("app {meta_id} has no store version — fetch owned apps first");
        }
        // Legible folder like the official launcher (games/bonelab), not the
        // bare numeric app id. --download <id> [dir] resumes into an existing
        // folder (a stalled run's half-written tree stays reusable via its hash cache).
        let name = self.store.apps[idx].name.clone();
        let dest = match dest_override {
            Some(d) => PathBuf::from(d),
            None => PathBuf::from(&self.config.download_dir).join(folder_name(&name)),
        };
        println!("installing into {}", dest.display());
        match fetcher::download_binary(
            &self.session.token,
            &binary_id,
            &dest,
            &std::sync::atomic::AtomicBool::new(false),
            &|done, total, bytes, total_bytes| {
                println!(
                    "{done}/{total} files · {} of {}",
                    human_bytes(bytes),
                    human_bytes(total_bytes)
                );
            },
        ) {
            Ok((n, reused)) => match pick_exe(&dest) {
                // Same rule as the GUI path: a folder without an exe is NOT
                // registered installed — Play would spawn an empty path.
                Some(exe) => {
                    let e = &mut self.store.apps[idx];
                    e.install_dir = dest.display().to_string();
                    e.launch_exe = exe.clone();
                    // A resume after a cancelled run left partial=true; a full
                    // verified install must clear it or the card keeps saying
                    // "partial download" with Play disabled.
                    e.partial = false;
                    self.save_store();
                    if reused == n {
                        format!("{n} files verified on disk, nothing re-downloaded, exe: {exe}")
                    } else if reused > 0 {
                        format!(
                            "{n} files installed ({reused} already on disk, not re-downloaded), exe: {exe}"
                        )
                    } else {
                        format!("{n} files installed, exe: {exe}")
                    }
                }
                None => format!(
                    "download finished but no exe found in {} — press Rescan folders",
                    dest.display()
                ),
            },
            Err(err) => {
                // A failed download leaves a half folder; mark it partial so
                // Rescan never presents it as a playable game.
                self.store.apps[idx].partial = true;
                self.save_store();
                format!("download failed: {err}")
            }
        }
    }

    /// Fetch release channels (LIVE / beta / custom) for the selected app.
    fn list_channels(&mut self) {
        if self.session.token.is_empty() {
            self.status = "not signed in yet — log in first".into();
            return;
        }
        if self.ch_inflight {
            self.status = "channels already loading".into();
            return;
        }
        let idx = self.selected;
        let app_id = match self.store.apps.get(idx) {
            Some(g) if !g.meta_app_id.is_empty() => g.meta_app_id.clone(),
            Some(g) => g.app_id.clone(),
            None => {
                self.status = "no game selected".into();
                return;
            }
        };

        self.ch_key = app_id.clone();
        self.ch_inflight = true;
        self.status = format!("loading versions for {app_id}…");
        let token = self.session.token.clone();
        let slot = self.ch_slot.clone();
        let repaint = self.ctx.clone();
        std::thread::spawn(move || {
            let res = fetcher::release_channels(&token, &app_id);
            *slot.lock().unwrap() = Some(res);
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    fn poll_channels(&mut self) {
        if !self.ch_inflight {
            return;
        }
        let Some(res) = self.ch_slot.lock().unwrap().take() else {
            return;
        };
        self.ch_inflight = false;
        match res {
            Ok(ch) => {
                // Resolve by app id: a rescan while this was loading may have
                // re-sorted the library, and a positional index would write the
                // channel list onto a DIFFERENT game.
                // Resolve by app id ONLY: a positional fallback would stamp
                // this channel list onto whatever game now sits at the old
                // index and persist it to disk.
                let idx = self
                    .store
                    .apps
                    .iter()
                    .position(|e| e.app_id == self.ch_key || e.meta_app_id == self.ch_key);
                match idx {
                    Some(i) => {
                        self.store.apps[i].branches = ch.clone();
                        self.save_store();
                    }
                    None => {
                        self.status = "this game is no longer in the library".into();
                        return;
                    }
                }
                // Keep the branch the user picked while this was loading; only
                // clamp when the fresh list is shorter than the old selection.
                self.selected_branch = self.selected_branch.min(ch.len().saturating_sub(1));
                if ch.is_empty() {
                    self.status = "no release channels reported for this game".into();
                } else {
                    // Plain verdict first: "update available" or
                    // "no update available", with the channel names after.
                    let entry = self
                        .store
                        .apps
                        .iter()
                        .position(|e| e.app_id == self.ch_key || e.meta_app_id == self.ch_key);
                    let verdict = match entry {
                        Some(i) => update_verdict(&self.store.apps[i], self.selected_branch),
                        None => "this game is no longer in the library".to_string(),
                    };
                    self.status = format!(
                        "{verdict} · channels: {}",
                        ch.iter()
                            .map(|c| format!("{} ({})", c.name, c.version))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
            Err(e) => self.status = format!("channels failed: {e}"),
        }
    }

    /// Offline integrity check of files downloaded earlier, off the UI thread.
    fn check_files(&mut self) {
        let idx = self.selected;
        let dir = match self.store.apps.get(idx) {
            Some(g) if !g.install_dir.is_empty() => PathBuf::from(&g.install_dir),
            Some(_) => {
                self.status = "selected game has no install dir, download it first".into();
                return;
            }
            None => {
                self.status = "no game selected".into();
                return;
            }
        };
        if self.chk_inflight {
            self.status = "file check already running".into();
            return;
        }
        self.chk_inflight = true;
        self.status = "checking files…".to_string();
        let slot = self.chk_slot.clone();
        let repaint = self.ctx.clone();
        std::thread::spawn(move || {
            let res = fetcher::verify_files(&dir).map(|(ok, bad, missing)| {
                if bad == 0 && missing == 0 {
                    format!("files OK: {ok} verified")
                } else {
                    format!("files check: {ok} ok, {bad} corrupt, {missing} missing — install again to repair")
                }
            });
            *slot.lock().unwrap() = Some(
                res.map_err(|_e| "this download has no checksums saved — install again".into()),
            );
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    fn poll_check(&mut self) {
        if !self.chk_inflight {
            return;
        }
        let Some(res) = self.chk_slot.lock().unwrap().take() else {
            return;
        };
        self.chk_inflight = false;
        match res {
            Ok(msg) => self.status = msg,
            Err(e) => self.status = e,
        }
    }

    /// Latest binary of the selected channel vs what is installed.
    fn update_available(&self) -> Option<String> {
        let g = self.store.apps.get(self.selected)?;
        let b = g.branches.get(self.selected_branch)?;
        if b.version_code > g.version_code {
            Some(format!("{} -> {} ({})", g.version, b.name, b.version))
        } else {
            None
        }
    }

    fn fetch_owned(&mut self) {
        if self.session.token.is_empty() {
            self.status = "not signed in yet — log in first".into();
            return;
        }
        if self.fetch_inflight {
            self.status = "library refresh already running".into();
            return;
        }
        // The store call is several slow HTTP round-trips; running it on the
        // UI thread freezes the window, so fetch on a worker and merge on
        // the next frame.
        auth::log_line("fetching owned apps in background");
        let token = self.session.token.clone();
        self.fetch_token = token.clone();
        let slot = self.fetch_result.clone();
        let covers = self.data_root().join("covers");
        let repaint = self.ctx.clone();
        // Purge any leftover result from a cancelled fetch.
        *self.fetch_result.lock().unwrap() = None;
        self.fetch_inflight = true;
        std::thread::spawn(move || {
            let result = match fetcher::owned_apps(&token, 400) {
                Ok(mut apps) => {
                    for info in &mut apps {
                        if info.cover_url.is_empty() {
                            // Entitlement nodes do not always carry a cover;
                            // the store format doc does.
                            if let Ok(meta) = fetcher::app_info(&token, &info.id) {
                                info.cover_url = meta.cover_url;
                            }
                        }
                        if !info.cover_url.is_empty() {
                            let dest = covers.join(format!("{id}.img", id = info.id));
                            fetcher::download_cover(&info.cover_url, &dest).ok();
                        }
                    }
                    Ok(apps)
                }
                Err(e) => Err(e),
            };
            *slot.lock().unwrap() = Some((token, result));
            if let Some(c) = repaint {
                c.request_repaint();
            }
        });
    }

    /// Merge the background fetch result once it lands.
    fn poll_fetch(&mut self) {
        if !self.fetch_inflight {
            return;
        }
        let done = self.fetch_result.lock().unwrap().take();
        if let Some((token, result)) = done {
            if token != self.session.token {
                // A fetch started for a bearer the user already replaced: merging
                // it would show the OLD account's games, and Install on them
                // would fail at the CDN. If a fetch for the CURRENT token is
                // still running, keep waiting for its result; otherwise nothing
                // waits for this stale one, so release the flag instead of
                // freezing the library loader forever.
                if self.fetch_token != self.session.token {
                    self.fetch_inflight = false;
                }
                return;
            }
            self.fetch_inflight = false;
            match result {
                Ok(apps) => {
                    let mut added = 0;
                    let cut = apps.len() >= 400;
                    let fetched_ids: Vec<String> = apps.iter().map(|a| a.id.clone()).collect();
                    for info in apps {
                        if let Some(existing) = self
                            .store
                            .apps
                            .iter_mut()
                            .find(|g| g.meta_app_id == info.id)
                        {
                            if existing.cover_url != info.cover_url {
                                existing.cover_url = info.cover_url.clone();
                                // Drop the cached texture so the new art loads.
                                self.tex_cache.remove(&info.id);
                            }
                            existing.version = info.version.clone();
                            existing.version_code = info.version_code;
                            existing.binary_id = info.binary_id.clone();
                            existing.description = info.description.clone();
                            existing.entitled = existing.entitled || info.entitled;
                            continue;
                        }
                        self.store.apps.push(GameEntry {
                            description: info.description.clone(),
                            app_id: info.id.clone(),
                            name: info.display_name,
                            install_dir: String::new(),
                            launch_exe: String::new(),
                            version: info.version,
                            meta_app_id: info.id.clone(),
                            binary_id: info.binary_id,
                            cover_url: info.cover_url,
                            entitled: info.entitled,
                            last_played: 0,
                            partial: false,
                            version_code: info.version_code,
                            branches: Vec::new(),
                            proton: String::new(),
                            launch_args: Vec::new(),
                            launch_env: Vec::new(),
                        });
                        added += 1;
                    }
                    // Entries the fresh fetch did not return have no PC build
                    // (device-tier entitlements) or left the account, keeping
                    // them listed meant Install 404ed at the CDN. Hand-installed
                    // entries (no meta_app_id) are untouched.
                    let before = self.store.apps.len();
                    self.store.apps.retain(|g| {
                        g.meta_app_id.is_empty() || fetched_ids.contains(&g.meta_app_id)
                    });
                    let dropped = before - self.store.apps.len();
                    if dropped > 0 {
                        self.open_game = None;
                        self.uninstall_ask = None;
                        self.melon_popup = None;
                        self.install_prompt = None;
                        self.tweak_popup = None;
                        #[cfg(not(target_os = "windows"))]
                        {
                            self.game_proton_popup = None;
                        }
                        self.custom_pick = None;
                        self.selected = 0;
                        self.selected_branch = 0;
                        auth::log_line(&format!("dropped {dropped} entries with no PC build"));
                    }
                    self.save_store();
                    self.status = if cut {
                        format!("owned apps: +{added} added; the store caps the list at 400, so a few may be hidden")
                    } else if dropped > 0 {
                        format!(
                            "owned apps: +{added} added; {dropped} removed (no PC build for them)"
                        )
                    } else if added == 0 {
                        // A refresh that found nothing new must not read like
                        // something was added.
                        "owned apps: no new games in library".into()
                    } else {
                        format!("owned apps: +{added} added to library")
                    };
                }
                Err(e) => self.status = format!("owned fetch failed: {e}"),
            }
        }
    }

    /// Config writes must follow the save-on-change convention (token field,
    /// store-id): picking a Proton runtime then leaving Settings used to lose
    /// the choice unless "Save config" was pressed.
    fn save_config(&self) {
        if let Ok(s) = serde_json::to_string_pretty(&self.config) {
            std::fs::write(&self.config_path, s).ok();
        }
    }

    fn save_store(&self) {
        if let Ok(s) = serde_json::to_string_pretty(&self.store) {
            std::fs::write(&self.store_path, s).ok();
        }
    }

    /// Add a game folder that already exists on disk (a rip, a Steam install,
    /// anything with a Windows exe inside). One-time import, the folder stays
    /// where it is; launch works through the same Proton/Wine path as installed
    /// games. Scans up to three levels deep so picking a parent folder finds
    /// the exe in a subfolder.
    fn import_folder(&mut self, dir: PathBuf) {
        let mut stack = vec![(dir.clone(), 0usize)];
        let mut exe: Option<PathBuf> = None;
        while let Some((d, depth)) = stack.pop() {
            if depth > 3 {
                continue;
            }
            let Ok(rd) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in rd.flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push((path, depth + 1));
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if name.ends_with(".exe")
                    && !name.contains("crashhandler")
                    && !name.contains("setup")
                    && !name.contains("installer")
                {
                    if name.ends_with("_windows64.exe") {
                        exe = Some(path.clone());
                        stack.clear();
                        break;
                    }
                    if exe.is_none() {
                        exe = Some(path.clone());
                    }
                }
            }
        }
        let Some(found) = exe else {
            self.status = format!(
                "no Windows exe found in {} (checked three levels deep)",
                short_path(&dir.display().to_string())
            );
            auth::log_line(&format!("import: no exe under {}", dir.display()));
            return;
        };
        let game_dir = found.parent().unwrap_or(Path::new(".")).to_path_buf();
        let text = game_dir.display().to_string();
        if self
            .store
            .apps
            .iter()
            .any(|g| norm_path(&g.install_dir) == norm_path(&text))
        {
            self.status = "that folder is already in the library".into();
            return;
        }
        // .oculus-manifest.json (written by the store installer) carries the
        // real app id: a folder containing it IS a store game, just copied.
        let auto = oculus_manifest(&game_dir)
            .and_then(|(app, _, _, _)| {
                self.store
                    .apps
                    .iter()
                    .position(|g| !g.meta_app_id.is_empty() && g.meta_app_id == app)
            })
            .or_else(|| auto_match_store(&self.store.apps, &game_dir));
        if let Some(i) = auto {
            self.attach_imported(i, &game_dir);
            return;
        }
        // No store identity found: ask which library game this is, with a
        // plain-folder escape hatch.
        self.import_ask = Some((game_dir.clone(), None));
        self.status = tr(&self.config, "import_match_title");
    }

    /// Bind an external folder to its library entry so it behaves exactly
    /// like a store-installed game: id, cover, update and verify all work.
    fn attach_imported(&mut self, idx: usize, dir: &Path) {
        let Some(found) = find_game_exe(dir) else {
            return;
        };
        let found = &found;
        let name = self.store.apps[idx].name.clone();
        let cover_url = self.store.apps[idx].cover_url.clone();
        let key = self.store.apps[idx].meta_app_id.clone();
        {
            let g = &mut self.store.apps[idx];
            g.install_dir = dir.display().to_string();
            g.launch_exe = match oculus_manifest(dir) {
                Some((_, _, launch, version)) if !launch.is_empty() => {
                    let exe = Path::new(&launch)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| {
                            found
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default()
                        });
                    if !version.is_empty() {
                        g.version = version;
                    }
                    exe
                }
                _ => found
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            };
            g.partial = false;
        }
        self.save_store();
        self.status = i18n::fill(&tr(&self.config, "import_matched"), &[("name", &name)]);
        auth::log_line(&format!(
            "import matched folder {} to store entry {}",
            dir.display(),
            self.store.apps[idx].meta_app_id
        ));
        // A matched entry may not have its cover cached locally yet.
        if !cover_url.is_empty() && !key.is_empty() {
            let covers = self.data_root().join("covers");
            let dest = covers.join(format!("{key}.img"));
            let repaint = self.ctx.clone();
            std::thread::spawn(move || {
                if dest.exists() {
                    return;
                }
                let _ = std::fs::create_dir_all(&covers);
                fetcher::download_cover(&cover_url, &dest).ok();
                if let Some(c) = repaint {
                    c.request_repaint();
                }
            });
        }
    }

    /// Import with no store identity: a plain folder entry, as before. The
    /// manifest canonical name (if present) labels it better than the dir.
    fn plain_import(&mut self, dir: &Path) {
        let game_dir = dir.to_path_buf();
        let Some(found) = find_game_exe(&game_dir) else {
            self.status = "no Windows exe found in that folder".into();
            return;
        };
        let text = game_dir.display().to_string();
        if self
            .store
            .apps
            .iter()
            .any(|g| norm_path(&g.install_dir) == norm_path(&text))
        {
            self.status = "that folder is already in the library".into();
            return;
        }
        let name = oculus_manifest(&game_dir)
            .map(|(_, n, _, _)| n)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                game_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "imported game".into())
            });
        let entry = GameEntry {
            description: String::new(),
            app_id: format!("local-{}", folder_name(&name)),
            name: name.clone(),
            install_dir: text.clone(),
            launch_exe: found
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            version: "from disk".into(),
            meta_app_id: String::new(),
            binary_id: String::new(),
            cover_url: String::new(),
            entitled: false,
            last_played: 0,
            version_code: 0,
            launch_args: Vec::new(),
            launch_env: Vec::new(),
            partial: false,
            branches: Vec::new(),
            proton: String::new(),
        };
        self.store.apps.push(entry.clone());
        self.save_store();
        auth::log_line(&format!(
            "import: {name} from {} (exe {})",
            game_dir.display(),
            found.display()
        ));
        self.status = i18n::fill(
            i18n::text(i18n::parse(&self.config.lang), "import_added").as_str(),
            &[("name", &name), ("dir", &short_path(&text))],
        );
    }

    fn rescan_dirs(&mut self) {
        let mut found: Vec<GameEntry> = Vec::new();
        let mut roots: Vec<PathBuf> = Vec::new();
        if !self.config.prefix_dir.is_empty() {
            // Prefix root: scan each game prefix's drive_c/games.
            let root = PathBuf::from(&self.config.prefix_dir);
            if let Ok(rd) = std::fs::read_dir(&root) {
                for sub in rd.flatten() {
                    if sub.path().is_dir() {
                        roots.push(sub.path().join("drive_c/games"));
                    }
                }
            }
        }
        // Anything Install put into the download dir shows up without the
        // user having to add it to the scan list.
        if !self.config.download_dir.is_empty() {
            roots.push(PathBuf::from(&self.config.download_dir));
        }
        // Steam's own library folders are scanned automatically: games
        // installed there should appear without configuring anything.
        let home = std::env::var("HOME").unwrap_or_default();
        for steam_root in [
            PathBuf::from(&home).join(".local/share/Steam"),
            PathBuf::from(&home).join(".steam/root"),
            PathBuf::from(&home).join(".steam/steam"),
        ] {
            let games = steam_root.join("steamapps/common");
            if games.is_dir() {
                roots.push(games);
            }
        }
        // Store entries with no install_dir still own folders in the download
        // dir (folder_name(name) or the numeric app id). Match scanned dirs to
        // them first, and register the folder onto the store entry instead of
        // pushing a twin local card.
        let mut owned_dirs: Vec<(PathBuf, usize)> = Vec::new();
        if !self.config.download_dir.is_empty() {
            let base = PathBuf::from(&self.config.download_dir);
            for (i, a) in self.store.apps.iter().enumerate() {
                // Only store entries with a store id that have NOT registered
                // a folder yet own an unregistered dir in the download dir.
                if !a.install_dir.is_empty() || a.meta_app_id.is_empty() {
                    continue;
                }
                for cand in [base.join(folder_name(&a.name)), base.join(&a.app_id)] {
                    owned_dirs.push((cand, i));
                }
            }
        }
        for root in roots {
            if !root.is_dir() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&root) {
                for e in entries.flatten() {
                    let p = e.path();
                    if !p.is_dir() {
                        continue;
                    }
                    let mut exes: Vec<PathBuf> = Vec::new();
                    if let Ok(files) = std::fs::read_dir(&p) {
                        for f in files.flatten() {
                            let fp = f.path();
                            if fp.extension().map(|x| x == "exe").unwrap_or(false) {
                                let n = fp
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_lowercase();
                                if n.contains("crashhandler") || n.contains("setup") {
                                    continue;
                                }
                                // Wine/Proton binaries are compatibility
                                // plumbing, not games: indexing them filled
                                // the library with junk "games" like wine64.
                                if n.contains("wine") || n.contains("proton") || n == "steam.exe" {
                                    continue;
                                }
                                exes.push(fp);
                            }
                        }
                    }
                    let dir_l = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase();
                    if dir_l.contains("compatibilitytools")
                        || dir_l.contains("proton")
                        || dir_l.contains("wine")
                    {
                        continue;
                    }
                    let exe = exes
                        .into_iter()
                        .max_by_key(|fp| fp.metadata().map(|m| m.len()).unwrap_or(0))
                        .map(|fp| fp.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if exe.is_empty() {
                        continue;
                    }
                    let name = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    // A store entry that owns this folder but never registered
                    // it (cancelled/partial install) must be written up here,
                    // not duplicated as a second card claiming "installed".
                    let owned = owned_dirs.iter().any(|(path, _)| *path == p);
                    if owned {
                        continue;
                    }
                    found.push(GameEntry {
                        description: String::new(),
                        app_id: name.clone(),
                        name,
                        install_dir: p.to_string_lossy().to_string(),
                        launch_exe: exe,
                        version: String::new(),
                        meta_app_id: String::new(),
                        binary_id: String::new(),
                        cover_url: String::new(),
                        entitled: false,
                        last_played: 0,
                        partial: false,
                        version_code: 0,
                        branches: Vec::new(),
                        proton: String::new(),
                        launch_args: Vec::new(),
                        launch_env: Vec::new(),
                    });
                }
            }
        }

        // Merge by install_dir: a scan only refreshes the exe, it never blanks
        // the stored version of an entry that came from the store.
        let mut merged = self.store.apps.clone();
        // Register owned-but-unregistered folders onto their store entries.
        for (path, idx) in &owned_dirs {
            if !path.is_dir() {
                continue;
            }
            let Some(exe) = pick_exe(path) else {
                continue;
            };
            if let Some(e) = merged.get_mut(*idx) {
                e.install_dir = path.to_string_lossy().to_string();
                e.launch_exe = exe;
                e.partial = false;
            }
        }
        for g in found {
            if let Some(old) = merged.iter_mut().find(|a| a.install_dir == g.install_dir) {
                old.launch_exe = g.launch_exe;
            } else {
                merged.push(g);
            }
        }
        merged.sort_by(|a, b| a.name.cmp(&b.name));
        // Purge junk recorded before the filter existed: local entries whose
        // "game" was a Wine/Proton binary or sat in a compatibility folder.
        // Store entries (with a store id) are never touched.
        // One folder = one card: a local twin pointing at a folder a store
        // entry owns must go, or the library shows two "installed" cards and
        // removing the twin leaves the store card claiming installed.
        let owned: Vec<String> = merged
            .iter()
            .filter(|g| !g.meta_app_id.is_empty())
            .map(|g| g.install_dir.clone())
            .filter(|d| !d.is_empty())
            .collect();
        merged.retain(|g| {
            if !g.meta_app_id.is_empty() {
                return true;
            }
            if owned.contains(&g.install_dir) {
                return false;
            }
            let exe = g.launch_exe.to_lowercase();
            let dir = g.install_dir.to_lowercase();
            !(exe.contains("wine")
                || exe.contains("proton")
                || exe.ends_with("steam.exe")
                || dir.contains("compatibilitytools")
                || dir.contains("proton")
                || dir.contains("wine"))
        });
        self.store.apps = merged;
        self.save_store();
        // Count folders actually located, not store entries the scan never
        // found on disk.
        let located = self
            .store
            .apps
            .iter()
            .filter(|g| !g.install_dir.is_empty())
            .count();
        self.status = format!("{} games found", located);
        // The vector just got re-sorted and possibly shrunk by the junk purge,
        // so every index-keyed UI state is stale: an open Uninstall/Install/
        // MelonLoader popup would name and delete a DIFFERENT game, or panic
        // out of bounds. Drop them all instead of keeping stale indices.
        self.uninstall_ask = None;
        self.install_prompt = None;
        self.melon_popup = None;
        self.open_game = None;
        self.selected = 0;
        self.selected_branch = 0;
    }

    fn launch(&mut self) {
        #[cfg_attr(target_os = "windows", allow(unused_mut))]
        let Some(mut g) = self.store.apps.get(self.selected).cloned() else {
            self.status = "no game selected".into();
            return;
        };
        let exe = resolve_launch_path(&g.install_dir, &g.launch_exe);
        #[cfg(not(target_os = "windows"))]
        {
            // Per-game Proton override wins over the global pick.
            let proton_script = if g.proton.is_empty() {
                self.config.proton_selected.clone()
            } else {
                g.proton.clone()
            };
            let use_proton = !proton_script.is_empty();
            if !use_proton && self.config.wine_binary.trim().is_empty() {
                self.status = "no Wine or Proton set up — pick a Proton version first".into();
                auth::log_line("launch refused: no Wine or Proton configured");
                return;
            }
            let game_dir = exe.parent().unwrap_or(Path::new("."));
            let inner_bin = if use_proton {
                PathBuf::from(proton_script.clone())
            } else {
                PathBuf::from(self.config.wine_binary.clone())
            };
            let mut inner = Vec::new();
            if use_proton {
                inner.push("waitforexitandrun".to_string());
            }
            inner.push(exe.to_string_lossy().into_owned());
            inner.extend(launch_tweak_args(&g));
            // One prefix per game (prefixes/<game>), created on demand. A
            // Proton runscript OUTSIDE Steam aborts with "No compat data
            // path?" unless both STEAM_COMPAT variables point at a real
            // prefix dir — that is why launches produced nothing at all.
            let prefix = game_prefix_root(&self.config.prefix_dir).join(folder_name(&g.name));
            if std::fs::create_dir_all(&prefix).is_err() {
                self.status = format!("could not create the game folder at {}", prefix.display());
                return;
            }
            // Games run plainly: the mount-namespace sandbox was dropped (the
            // user asked for it - bwrap refused the runtime path on their box
            // and VR would likely be blocked too). Wine's Z: drive follows the
            // cwd, which is set to the game folder below, so the drive listing
            // itself starts inside the game's own directory.
            let mut cmd = std::process::Command::new(&inner_bin);
            for a in &inner {
                cmd.arg(a);
            }
            // Own process group so Stop can kill the whole tree: the proton
            // runscript, wine, wineserver and bridges all share this pgid.
            cmd.process_group(0);
            cmd.current_dir(game_dir);
            prefix_env(&mut cmd, &prefix);
            #[cfg(not(target_os = "windows"))]
            if use_proton {
                cmd.env("SteamAppId", steam_app_id_for(&g.name, &g.app_id));
                // Steam starts SteamVR itself before a VR game; outside Steam
                // nobody does, and Proton's bridge only connects to a live
                // vrserver (Valve issue #962). The launcher takes over both
                // jobs: persist PROTON_VR_RUNTIME once and bring vrserver up.
                if let Some(root) = steamvr_runtime_root(&std::env::var("HOME").unwrap_or_default())
                {
                    if !has_env_key(&g.launch_env, "PROTON_VR_RUNTIME") {
                        let mut env = g.launch_env.clone();
                        if toggle_env_entry(
                            &mut env,
                            "PROTON_VR_RUNTIME",
                            &root.to_string_lossy(),
                            true,
                        ) {
                            g.launch_env = env.clone();
                            if let Some(e) = self.store.apps.get_mut(self.selected) {
                                e.launch_env = env;
                                self.save_store();
                            }
                            auth::log_line(&format!(
                                "SteamVR found, PROTON_VR_RUNTIME enabled for game {} ({})",
                                folder_name(&g.name),
                                root.display()
                            ));
                        }
                    }
                    if has_env_key(&g.launch_env, "PROTON_VR_RUNTIME") {
                        if let Some(manifest) = steamxr_manifest(&root) {
                            auth::log_line(&format!(
                                "OpenXR runtime manifest pointed at {}",
                                manifest.display()
                            ));
                            cmd.env("XR_RUNTIME_JSON", manifest);
                        }
                        if vrserver_running_cached() {
                            auth::log_line("vrserver already running — the game bridges into it");
                            vr_netns_diag();
                            if !proton_vr_capable(&inner_bin) {
                                auth::log_line(
                                    "this Proton build cannot open XR outside Steam - Valve's wineopenxr only initializes the VR registry through Steam (issues #7382 and #8256); download GE-Proton 10-33 or newer in Proton versions and pick it for this game",
                                );
                            }
                        } else {
                            // SteamVR ships .desktop handlers for vrserver; the
                            // launcher never spawns vrserver itself. Pipes default
                            // to ~/.config/SteamVR when started outside Steam, and
                            // VR_RUNTIME_DIR below points the game at that dir.
                            auth::log_line("vrserver not running — start SteamVR via its desktop handler, then launch the game");
                        }
                        // Valve issue #962: the game's vrclient needs
                        // VR_RUNTIME_DIR to reach a vrserver it did not spawn
                        // through Steam. Steam's container keeps its own pipe
                        // dir hidden, so ss-discovered paths are forwarded when
                        // visible and ~/.config/SteamVR is the pinned default.
                        // SteamVR Linux escapes pressure-vessel (vrsetup.sh does the
                        // setcap dance outside the container), so vrserver's pipes
                        // live on the host and an outside app reaches them through
                        // SteamVR's default pipe dir, the same way Vivecraft does.
                        // Pinning a dir the server never used caused the
                        // XR_ERROR_RUNTIME_UNAVAILABLE mismatch, so the launcher
                        // only forwards VR_RUNTIME_DIR when the live vrserver
                        // advertises a visible one and otherwise leaves it unset.
                        let from_server =
                            vr_runtime_dir_from_server_env().or_else(vr_runtime_dir_discovered);
                        match &from_server {
                            Some(d) => {
                                cmd.env("VR_RUNTIME_DIR", d);
                                auth::log_line(&format!(
                                    "VR_RUNTIME_DIR forwarded from the live vrserver: {}",
                                    d.display()
                                ));
                            }
                            None => {
                                // Steam hands its VR apps VR_RUNTIME_DIR=/tmp;
                                // a vrserver started without an advertised dir
                                // uses that same documented default, so an
                                // outside game must ask for it explicitly or
                                // vrclient probes a dir it never used.
                                cmd.env("VR_RUNTIME_DIR", "/tmp");
                                let pipe_dir = std::path::Path::new("/tmp/steamvr");
                                auth::log_line(&format!(
                                    "VR_RUNTIME_DIR set to /tmp (Steam's documented default); pipe dir {} {}",
                                    pipe_dir.display(),
                                    if pipe_dir.exists() {
                                        "exists — vrserver's pipes are here"
                                    } else {
                                        "missing — vrserver binds abstract sockets instead"
                                    }
                                ));
                            }
                        }
                        // With the runtime live Unity renders to the VR compositor,
                        // not the desktop window: an empty window on the desktop is
                        // expected, the picture belongs to the headset.
                        auth::log_line("VR runtime wired: game renders into the VR compositor, connect the headset (Steam Link VR) to see it");
                    } else {
                        // Radio unchecked: leave the XR runtime undiscoverable so
                        // Unity falls back to flatscreen and the desktop window
                        // actually shows the game.
                        auth::log_line("SteamVR radio off — flatscreen launch, XR runtime not wired, desktop window renders the game");
                    }
                }
            }
            apply_launch_tweak_env(&mut cmd, &g);
            if let Some(shim) = platform_shim_step(game_dir) {
                auth::log_line(&format!("platform shim placed: {}", shim.display()));
                #[cfg(not(target_os = "windows"))]
                if use_proton
                    && steamvr_runtime_root(&std::env::var("HOME").unwrap_or_default()).is_some()
                {
                    if let Some(rt) = librevive_rt_step(game_dir) {
                        auth::log_line(&format!("revive translation placed as {}", rt.display()));
                    }
                }
                // The trace log is opt-in via the Shim trace radio in Launch
                // options: Bonelab polls PopMessage thousands of times per
                // session, an always-on trace filled 6 MB in the game folder.
            }
            #[cfg(not(target_os = "windows"))]
            if use_proton {
                for mirrored in openvr_pathreg_step(&prefix) {
                    auth::log_line(&format!(
                        "openvr path registry mirrored into prefix: {}",
                        mirrored.display()
                    ));
                }
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            match cmd.spawn() {
                Ok(child) => {
                    let via = if use_proton { "Proton" } else { "wine" };
                    let pid = child.id();
                    self.running_pid = Some(pid);
                    self.running_prefix = Some(prefix.clone());
                    self.running_exe = Some(exe.to_string_lossy().into_owned());
                    self.running_flag =
                        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                    auth::log_line(&format!(
                        "launch: pid {} via {} prefix {} exe {}",
                        pid,
                        via,
                        prefix.display(),
                        exe.display()
                    ));
                    capture_output(
                        child,
                        format!("{} via {}", g.name, via),
                        prefix.clone(),
                        self.launch_slot.clone(),
                        self.running_flag.clone(),
                    );
                    self.status = format!(
                        "launched pid {} via {} (prefix {})",
                        pid,
                        via,
                        prefix.display()
                    );
                    if std::env::var("WAYLAND_DISPLAY").is_ok() {
                        auth::log_line("launch: running under a Wayland session");
                    }
                }
                Err(e) => {
                    auth::log_line(&format!("launch failed: {e} — exe {}", exe.display()));
                    self.status = format!("launch failed: {e}");
                }
            };
        }
        #[cfg(target_os = "windows")]
        {
            let steamvr_root = steamvr_root_windows();
            let mut xr_json: Option<PathBuf> = None;
            if let Some(root) = &steamvr_root {
                let vr = root.join(r"steamapps\common\SteamVR");
                if let Some(m) = xr_manifest(&vr) {
                    auth::log_line(&format!(
                        "OpenXR runtime manifest pointed at {}",
                        m.display()
                    ));
                    xr_json = Some(m);
                }
                let xr_dll = self.data_root().join("LibReviveXR64.dll");
                std::fs::write(&xr_dll, REVIVE_XR_BYTES).ok();
                let game_dir_win = exe.parent().unwrap_or(Path::new("."));
                let beside = game_dir_win.join("LibOVRRT64_1.dll");
                if std::fs::copy(&xr_dll, &beside).is_ok() {
                    auth::log_line(&format!(
                        "revive translation placed as {}",
                        beside.display()
                    ));
                }
                if let Some(dir) = meta_runtime_dir() {
                    let impl_path = dir.join("LibOVRRTImpl64_1.dll");
                    let rt_path = dir.join("LibOVRRT64_1.dll");
                    if dll_matches(&impl_path, REVIVE_XR_BYTES.len() as u64) {
                        auth::log_line(
                            "vendor runtime already replaced, plugin games reach SteamVR through the launcher's translation layer",
                        );
                    } else {
                        let backup = self.data_root().join("iris-runtime-backup");
                        std::fs::create_dir_all(&backup).ok();
                        for n in ["LibOVRRT64_1.dll", "LibOVRRTImpl64_1.dll"] {
                            let o = dir.join(n);
                            let b = backup.join(n);
                            if o.is_file() && !b.is_file() {
                                std::fs::copy(&o, &b).ok();
                            }
                        }
                        let xr = xr_dll.to_string_lossy().to_string();
                        let rt_s = rt_path.to_string_lossy().to_string();
                        let impl_s = impl_path.to_string_lossy().to_string();
                        let pairs = [(xr.as_str(), rt_s.as_str()), (xr.as_str(), impl_s.as_str())];
                        if meta_runtime_swap(&pairs) {
                            // The elevated script copies asynchronously behind
                            // UAC. Poll the target file instead of freezing a
                            // fixed 6 s: the game proceeds the moment the
                            // swapped DLL lands, worst case still bounded.
                            let mut swapped = dll_matches(&impl_path, REVIVE_XR_BYTES.len() as u64);
                            for _ in 0..20 {
                                if swapped {
                                    break;
                                }
                                std::thread::sleep(std::time::Duration::from_millis(500));
                                swapped = dll_matches(&impl_path, REVIVE_XR_BYTES.len() as u64);
                            }
                            if swapped {
                                self.win_swap_landed = Some(true);
                                auth::log_line(
                                    "headset runtime replaced with the SteamVR translation layer: link slot takeover done",
                                );
                            } else {
                                self.win_swap_landed = Some(false);
                                auth::log_line(
                                    "runtime swap did not land — UAC was declined or the unsigned DLL was blocked",
                                );
                            }
                        } else {
                            auth::log_line("elevated runtime swap script could not start");
                        }
                    }
                }
                if vrserver_running_windows_cached() {
                    auth::log_line("vrserver already running — the game will connect to it");
                } else {
                    auth::log_line("vrserver not running — start SteamVR yourself (vrserver.exe or its shortcut), the launcher never launches it");
                }
            }
            if let Some(target) = revive_dll_step(
                &self.config.revive_dll,
                exe.parent().unwrap_or(Path::new(".")),
            ) {
                auth::log_line(&format!(
                    "revive layer: {} copied beside the game exe",
                    target.display()
                ));
            }
            if !self.config.ovr_server.trim().is_empty() {
                match revive_server_start(&self.config.ovr_server) {
                    Some(pid) => {
                        auth::log_line(&format!("revive layer: runtime started pid {pid}"))
                    }
                    None => {
                        auth::log_line("revive layer: runtime exe not found or could not start")
                    }
                }
            }
            if let Some(shim) = platform_shim_step(exe.parent().unwrap_or(Path::new("."))) {
                auth::log_line(&format!("platform shim placed: {}", shim.display()));
            }
            if let Some(pid) = revive_injector_start(&self.data_root().join("tools")) {
                auth::log_line(&format!("revive layer: injector started pid {pid}"));
            }
            let mut cmd = std::process::Command::new(&exe);
            cmd.current_dir(exe.parent().unwrap_or(Path::new(".")));
            if let Some(m) = &xr_json {
                cmd.env("XR_RUNTIME_JSON", m);
            }
            apply_launch_tweak_env(&mut cmd, &g);
            for a in launch_tweak_args(&g) {
                cmd.arg(a);
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            match cmd.spawn() {
                Ok(child) => {
                    let pid = child.id();
                    self.running_pid = Some(pid);
                    self.running_prefix = None;
                    self.running_exe = Some(exe.to_string_lossy().into_owned());
                    self.running_flag =
                        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                    auth::log_line(&format!("launch: pid {} exe {}", pid, exe.display()));
                    capture_output(
                        child,
                        format!("{} (direct)", g.name),
                        exe.parent().unwrap_or(Path::new(".")).to_path_buf(),
                        self.launch_slot.clone(),
                        self.running_flag.clone(),
                    );
                    self.status = if self.win_swap_landed == Some(false) {
                        format!("launched pid {pid}, runtime swap failed, see launcher.log")
                    } else {
                        format!("launched pid {}", pid)
                    };
                    self.win_swap_landed = None;
                }
                Err(e) => {
                    auth::log_line(&format!("launch failed: {e} — exe {}", exe.display()));
                    self.status = format!("launch failed: {e}");
                }
            }
        }
        if self.status.starts_with("launched") {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if let Some(e) = self.store.apps.get_mut(self.selected) {
                e.last_played = now;
            }
            self.save_store();
        }
    }

    /// Toggle one preset inside this game's stored launch options without
    /// rewriting the other entries, and persist the result right away.
    fn apply_preset(&mut self, idx: usize, preset: Preset, item: &str, on: bool) {
        match preset {
            Preset::Dll(_) => {
                let mut env = env_lines(&self.tweak_env);
                if !toggle_dll_override(&mut env, item, on) {
                    return;
                }
                self.tweak_env = env.join("\n");
            }
            Preset::Arg(_) => {
                let mut args = env_lines(&self.tweak_args);
                if !toggle_arg(&mut args, item, on) {
                    return;
                }
                self.tweak_args = args.join("\n");
            }
            #[cfg(not(target_os = "windows"))]
            Preset::SteamVr => {
                let home = std::env::var("HOME").unwrap_or_default();
                let Some(root) = steamvr_runtime_root(&home) else {
                    self.status =
                        "SteamVR install not found — install SteamVR for Linux first".into();
                    return;
                };
                let mut env = env_lines(&self.tweak_env);
                if !toggle_env_entry(&mut env, "PROTON_VR_RUNTIME", &root.to_string_lossy(), on) {
                    return;
                }
                self.tweak_env = env.join("\n");
            }
            Preset::Env(key) => {
                let dir = self.store.apps.get(idx).and_then(|g| {
                    if g.install_dir.is_empty() {
                        None
                    } else {
                        Some(g.install_dir.clone())
                    }
                });
                let Some(dir) = dir else {
                    self.status = "game folder not known — install the game first".into();
                    return;
                };
                let value = format!("{}/iris-shim-trace.log", dir.trim_end_matches('/'));
                let mut env = env_lines(&self.tweak_env);
                if !toggle_env_entry(&mut env, key, &value, on) {
                    return;
                }
                self.tweak_env = env.join("\n");
            }
        }
        if let Some(e) = self.store.apps.get_mut(idx) {
            e.launch_env = env_lines(&self.tweak_env);
            e.launch_args = env_lines(&self.tweak_args);
            self.save_store();
        }
        auth::log_line(&format!(
            "launch option {}: {item}",
            if on { "on" } else { "off" }
        ));
        self.status = format!("{} {item}", if on { "enabled" } else { "removed" });
    }

    /// Close the running game and everything Wine/Proton started under it.
    /// Soft close sends SIGTERM / taskkill without /F so the game can exit
    /// cleanly; hard kill takes the whole process group with SIGKILL, plus
    /// pkill on the prefix because wineserver escapes the group. Neither
    /// clears the running state here: it ends when the process is really
    /// gone, so the Running button stays up until then.
    fn stop_running(&mut self, hard: bool) {
        let Some(pid) = self.running_pid else {
            self.running_flag
                .store(false, std::sync::atomic::Ordering::Relaxed);
            return;
        };
        #[cfg(not(target_os = "windows"))]
        {
            let sig = if hard { "-9" } else { "-TERM" };
            let _ = std::process::Command::new("kill")
                .arg(sig)
                .arg(format!("-{pid}"))
                .spawn();
            let _ = std::process::Command::new("kill")
                .arg(sig)
                .arg(pid.to_string())
                .spawn();
            if let Some(prefix) = self.running_prefix.clone() {
                // wineserver names the prefix in its cmdline and can live
                // outside the group; signalling it takes the prefix's wine tree.
                let _ = std::process::Command::new("pkill")
                    .arg(sig)
                    .arg("-f")
                    .arg(prefix.display().to_string())
                    .spawn();
            }
            if let Some(exe) = self.running_exe.clone() {
                // The game process inside wine carries the exe path in its
                // cmdline; the group signal misses it when Proton detached
                // the session, so match the path directly.
                let _ = std::process::Command::new("pkill")
                    .arg(sig)
                    .arg("-f")
                    .arg(exe)
                    .spawn();
            }
        }
        #[cfg(target_os = "windows")]
        {
            if hard {
                let _ = std::process::Command::new("taskkill")
                    .args(["/T", "/F", "/PID", &pid.to_string()])
                    .spawn();
            } else {
                let _ = std::process::Command::new("taskkill")
                    .args(["/T", "/PID", &pid.to_string()])
                    .spawn();
            }
        }
        auth::log_line(&format!(
            "{} requested for pid {pid} and its processes",
            if hard { "hard kill" } else { "soft close" }
        ));
        self.status = if hard {
            "killing game..."
        } else {
            "closing game..."
        }
        .into();
    }

    /// The Running state ends when no game process survives, not when a stop
    /// button was pressed. The Proton wrapper pid dies early while wine keeps
    /// the game in a detached session, so the check is pid PLUS a pgrep/tasklist
    /// pattern on the exe path; a soft close the game ignores keeps Running
    /// shown until a hard kill takes every matching process down.
    fn poll_running_liveness(&mut self) {
        let Some(pid) = self.running_pid else { return };
        let mut alive = pid_alive(pid);
        if let Some(exe) = self.running_exe.clone() {
            #[cfg(not(target_os = "windows"))]
            {
                if !alive {
                    let out = std::process::Command::new("pgrep")
                        .arg("-f")
                        .arg(&exe)
                        .output();
                    if let Ok(o) = out {
                        if !String::from_utf8_lossy(&o.stdout).trim().is_empty() {
                            alive = true;
                        }
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                let name = std::path::Path::new(&exe)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !alive && !name.is_empty() {
                    let out = std::process::Command::new("tasklist")
                        .args(["/FI", &format!("IMAGENAME eq {name}")])
                        .output();
                    if let Ok(o) = out {
                        if String::from_utf8_lossy(&o.stdout).contains(&name) {
                            alive = true;
                        }
                    }
                }
            }
        }
        if !alive {
            self.running_pid = None;
            self.running_prefix = None;
            self.running_exe = None;
            self.running_flag
                .store(false, std::sync::atomic::Ordering::Relaxed);
            auth::log_line(&format!(
                "game process {pid} and every exe-pattern match is gone"
            ));
            self.status = "game stopped".into();
        }
    }

    /// Turn a captured graphics-init failure into advice instead of silence.
    fn poll_launch(&mut self) {
        let Some(res) = self.launch_slot.lock().unwrap().take() else {
            return;
        };
        if res.as_deref() == Ok("prefix") {
            self.status = tr(&self.config, "gfx_hint_prefix");
            auth::log_line("prefix version mismatch; reset the prefix advice shown");
            return;
        }
        if res.is_ok() {
            let key = if std::env::var("WAYLAND_DISPLAY").is_ok() {
                "gfx_hint_wayland"
            } else {
                "gfx_hint_plain"
            };
            self.status = tr(&self.config, key);
            auth::log_line("graphics init failed; advice shown in status");
        }
    }

    /// Run an exe the user picked inside this game's folder, through the same
    /// Proton/Wine prefix the game itself uses (Linux) or directly (Windows).
    fn run_in_prefix(&mut self, idx: usize, exe: &Path) {
        let Some(g) = self.store.apps.get(idx).cloned() else {
            self.status = "this game is no longer in the library".into();
            return;
        };
        #[cfg(not(target_os = "windows"))]
        {
            let prefix = game_prefix_root(&self.config.prefix_dir).join(folder_name(&g.name));
            if std::fs::create_dir_all(&prefix).is_err() {
                self.status = format!("could not create the game folder at {}", prefix.display());
                return;
            }
            let proton_script = if g.proton.is_empty() {
                self.config.proton_selected.clone()
            } else {
                g.proton.clone()
            };
            #[cfg(not(target_os = "windows"))]
            let steam_app_id = steam_app_id_for(&g.name, &g.app_id);
            let (inner_bin, inner) = if !g.proton.is_empty() {
                (
                    PathBuf::from(g.proton.clone()),
                    vec![
                        "waitforexitandrun".to_string(),
                        exe.to_string_lossy().into_owned(),
                    ],
                )
            } else if !self.config.proton_selected.is_empty() {
                (
                    PathBuf::from(self.config.proton_selected.clone()),
                    vec![
                        "waitforexitandrun".to_string(),
                        exe.to_string_lossy().into_owned(),
                    ],
                )
            } else if !self.config.wine_binary.trim().is_empty() {
                (
                    PathBuf::from(self.config.wine_binary.clone()),
                    vec![exe.to_string_lossy().into_owned()],
                )
            } else {
                self.status = "no Wine or Proton set up — pick a Proton version first".into();
                return;
            };
            let mut cmd = std::process::Command::new(&inner_bin);
            for a in &inner {
                cmd.arg(a);
            }
            cmd.process_group(0);
            cmd.current_dir(exe.parent().unwrap_or(Path::new(".")));
            prefix_env(&mut cmd, &prefix);
            #[cfg(not(target_os = "windows"))]
            if !g.proton.is_empty() || !self.config.proton_selected.is_empty() {
                cmd.env("SteamAppId", &steam_app_id);
            }
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            match cmd.spawn() {
                Ok(child) => {
                    let pid = child.id();
                    self.running_pid = Some(pid);
                    self.running_prefix = Some(prefix.clone());
                    self.running_exe = Some(exe.to_string_lossy().into_owned());
                    self.running_flag =
                        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                    let via = if !proton_script.is_empty() {
                        "Proton"
                    } else {
                        "wine"
                    };
                    auth::log_line(&format!(
                        "custom run: pid {} via {} prefix {} exe {}",
                        pid,
                        via,
                        prefix.display(),
                        exe.display()
                    ));
                    capture_output(
                        child,
                        format!("{} custom exe via {}", g.name, via),
                        prefix.clone(),
                        self.launch_slot.clone(),
                        self.running_flag.clone(),
                    );
                    self.status = format!(
                        "launched pid {} via {} (prefix {})",
                        pid,
                        via,
                        prefix.display()
                    );
                }
                Err(e) => self.status = format!("launch failed: {e}"),
            }
        }
        #[cfg(target_os = "windows")]
        {
            let mut cmd = std::process::Command::new(exe);
            cmd.current_dir(exe.parent().unwrap_or(Path::new(".")));
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            match cmd.spawn() {
                Ok(child) => {
                    let pid = child.id();
                    self.running_pid = Some(pid);
                    self.running_prefix = None;
                    self.running_flag =
                        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                    auth::log_line(&format!("custom run: pid {} exe {}", pid, exe.display()));
                    capture_output(
                        child,
                        format!("{} custom exe", g.name),
                        exe.parent().unwrap_or(Path::new(".")).to_path_buf(),
                        self.launch_slot.clone(),
                        self.running_flag.clone(),
                    );
                    self.status = format!("launched pid {}", pid);
                }
                Err(e) => self.status = format!("launch failed: {e}"),
            }
        }
    }

    /// Delete this game's Wine/Proton prefix. Proton rebuilds it on the next
    /// launch — needed when a prefix was made by a different Proton build
    /// ("Prefix has an invalid version?!"). Saves inside the prefix go away
    /// with it, which is why it lives behind a menu click, not silently.
    fn reset_prefix(&mut self, idx: usize) {
        #[cfg(not(target_os = "windows"))]
        {
            let Some(g) = self.store.apps.get(idx) else {
                return;
            };
            let prefix = game_prefix_root(&self.config.prefix_dir).join(folder_name(&g.name));
            if !prefix.exists() {
                self.status = tr(&self.config, "prefix_reset_none");
                return;
            }
            match std::fs::remove_dir_all(&prefix) {
                Ok(()) => {
                    self.status = tr(&self.config, "prefix_reset_done");
                    auth::log_line(&format!("prefix removed: {}", prefix.display()));
                }
                Err(e) => self.status = format!("could not remove the prefix: {e}"),
            }
        }
        #[cfg(target_os = "windows")]
        {
            let _ = idx;
            self.status =
                "prefixes belong to Wine/Proton runs; Windows launches run the exe directly".into();
        }
    }

    /// Open the Wine configurator against this game's prefix (Linux only).
    #[cfg(not(target_os = "windows"))]
    fn run_winecfg(&mut self, idx: usize) {
        let Some(g) = self.store.apps.get(idx).cloned() else {
            self.status = "this game is no longer in the library".into();
            return;
        };
        let prefix = game_prefix_root(&self.config.prefix_dir).join(folder_name(&g.name));
        if std::fs::create_dir_all(&prefix).is_err() {
            self.status = format!("could not create the game folder at {}", prefix.display());
            return;
        }
        // A game can have its own Proton picked in the ⋯ menu; that build's
        // bundled wine is the fallback too, not just the Settings choice.
        let proton = if g.proton.is_empty() {
            self.config.proton_selected.clone()
        } else {
            g.proton.clone()
        };
        let Some((exe, args)) = wine_settings_plan(&self.config.wine_binary, &proton) else {
            self.status = tr(&self.config, "wine_settings_missing");
            return;
        };
        let game_dir = PathBuf::from(g.install_dir.clone());
        let game_dir = if game_dir.is_dir() {
            game_dir
        } else {
            prefix.clone()
        };
        let game_dir_unused = &game_dir;
        let _ = game_dir_unused;
        let mut cmd = std::process::Command::new(&exe);
        for a in &args {
            cmd.arg(a);
        }
        cmd.env("WINEPREFIX", &prefix);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                auth::log_line(&format!("winecfg: pid {} prefix {}", pid, prefix.display()));
                self.status = i18n::fill(
                    i18n::text(i18n::parse(&self.config.lang), "wine_settings_opened").as_str(),
                    &[("pid", &pid.to_string())],
                );
            }
            Err(e) => self.status = format!("wine settings failed: {e}"),
        }
    }

    /// Prefer the desktop's own folder dialog; fall back to the built-in
    /// chooser only when no system dialog exists on this machine.
    fn pick_dir(&mut self, start: PathBuf, target: DirTarget) {
        match system_pick(&start, true) {
            Some(Some(path)) => {
                let text = path.display().to_string();
                match target {
                    DirTarget::Install => {
                        if let Some(pp) = self.install_prompt.as_mut() {
                            pp.dir = text.clone();
                        }
                    }
                    DirTarget::DownloadDir => {
                        self.config.download_dir = text.clone();
                        self.save_config();
                    }
                    DirTarget::PrefixDir => {
                        self.config.prefix_dir = text.clone();
                        self.save_config();
                    }
                    DirTarget::Import => {
                        self.import_folder(path.clone());
                        return;
                    }
                }
                self.status = format!("folder set to {text}");
            }
            Some(None) => {
                self.status = "folder choice cancelled".into();
            }
            None => {
                self.status = "no system folder dialog here, using the built-in picker".into();
                self.dir_popup = Some((start, target));
            }
        }
    }

    /// Delete installed game files after the confirmation popup.
    fn uninstall(&mut self, idx: usize) {
        // Bounds-check: a rescan can shrink/re-sort the library while this
        // popup is open, and indexing blindly would panic or delete a folder
        // that now belongs to a DIFFERENT game.
        let Some(entry) = self.store.apps.get(idx) else {
            self.uninstall_ask = None;
            self.status = "this game is no longer in the library".into();
            return;
        };
        let dir = entry.install_dir.clone();
        if dir.is_empty() {
            self.status = "nothing installed to remove".into();
            return;
        }
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {
                let name = self.store.apps[idx].name.clone();
                self.store.apps[idx].install_dir.clear();
                self.store.apps[idx].partial = false;
                self.store.apps[idx].launch_exe.clear();
                self.save_store();
                self.status = format!("{name} removed from disk");
            }
            Err(e) => self.status = format!("could not remove {dir}: {e}"),
        }
    }

    /// Dedicated page for one game: art, Play/Install, and the ⋯ menu with
    /// update, verify, browse files, uninstall, versions, MelonLoader and a
    /// per-game Proton choice.
    fn game_page(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, idx: usize) {
        let g = self.store.apps[idx].clone();
        // A half-downloaded folder is not installed: partial games offer
        // Install (which reuses intact files), never Play/Verify/Uninstall-as-installed.
        let installed = !g.install_dir.is_empty() && !g.partial;
        ui.horizontal(|ui| {
            // No arrow glyph in egui's bundled font; plain words instead.
            if ui.button("Back to library").clicked() {
                self.open_game = None;
                // An Uninstall popup belongs to the game page; leaving it open
                // across navigation keeps it pointed at a stale index.
                self.uninstall_ask = None;
            }
            ui.heading(&g.name);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.dots_tex.is_none() {
                    // RGBA bytes directly: transparent background, three white dots.
                    let mut bytes = vec![0u8; 24 * 16 * 4];
                    for (cx, cy) in [(5, 8), (12, 8), (19, 8)] {
                        for y in 0..16 {
                            for x in 0..24 {
                                let dx = x as f32 - cx as f32;
                                let dy = y as f32 - cy as f32;
                                if dx * dx + dy * dy <= 6.0 {
                                    let i = (y * 24 + x) * 4;
                                    bytes[i] = 220;
                                    bytes[i + 1] = 220;
                                    bytes[i + 2] = 220;
                                    bytes[i + 3] = 220;
                                }
                            }
                        }
                    }
                    let img = egui::ColorImage::from_rgba_unmultiplied([24, 16], &bytes);
                    self.dots_tex =
                        Some(ctx.load_texture("dots-menu", img, egui::TextureOptions::default()));
                }
                let dots = self.dots_tex.clone();
                if let Some(dots) = dots {
                    ui.menu_image_button(&dots, |ui| {
                    let newer = self.update_available().is_some();
                    // Mirrors open_install's refusals: a clickable button that
                    // always errors is a lying button.
                    ui.add_enabled_ui(self.installable(idx), |ui| {
                        if ui
                            .button(if installed {
                                if newer {
                                    "Update"
                                } else {
                                    "Reinstall"
                                }
                            } else {
                                "Install"
                            })
                            .clicked()
                        {
                            self.open_install();
                            ui.close_menu();
                        }
                    });
                    // While a download is writing this folder, repair actions
                    // would fight it: Verify/Uninstall must stay disabled.
                    let dl_busy = self.dl_busy(idx);
                    ui.add_enabled_ui(!dl_busy, |ui| {
                        if installed && ui.button(tr(&self.config, "verify_files")).clicked() {
                            self.check_files();
                            ui.close_menu();
                        }
                        if installed && ui.button(tr(&self.config, "browse_files")).clicked() {
                            open_folder(Path::new(&g.install_dir));
                            ui.close_menu();
                        }
                        // The Unity data folder (saves, Player.log) lives in
                        // the Wine prefix on Linux and in LOCALAPPDATA on
                        // Windows; both open with the system file manager.
                        if installed && ui.button(tr(&self.config, "open_data_folder")).clicked() {
                            #[cfg(not(target_os = "windows"))]
                            let prefix = game_prefix_root(&self.config.prefix_dir).join(folder_name(&g.name));
                            #[cfg(target_os = "windows")]
                            let prefix = PathBuf::new();
                            #[cfg(target_os = "windows")]
                            let _ = (&self.config, idx);
                            match data_folder(Some(&prefix)) {
                                Some(dir) => {
                                    open_folder(&dir);
                                    self.status = format!("opened {}", dir.display());
                                }
                                None => {
                                    self.status = "data folder not there yet — it appears after the game first runs".into();
                                }
                            }
                            ui.close_menu();
                        }
                        // A partial entry may have files on disk even with no
                        // install dir registered; an empty dir would make the
                        // popup's button always answer "nothing installed".
                        if (installed || !g.install_dir.is_empty())
                            && ui.button(tr(&self.config, "uninstall")).clicked()
                        {
                            self.uninstall_ask = Some(idx);
                            ui.close_menu();
                        }
                        // Run any other exe from this folder inside the game's
                        // prefix (mod managers, patchers, config tools).
                        if !g.install_dir.is_empty()
                            && ui.button(tr(&self.config, "run_other_exe")).clicked()
                        {
                            let dir = PathBuf::from(&g.install_dir);
                            match system_pick(&dir, false) {
                                Some(Some(exe)) => self.run_in_prefix(idx, &exe),
                                Some(None) => {
                                    self.status = "exe choice cancelled".into();
                                }
                                None => {
                                    // No system dialog: show the folder's own
                                    // exe list instead of refusing silently.
                                    self.custom_pick = Some(idx);
                                }
                            }
                            ui.close_menu();
                        }
                        if !g.install_dir.is_empty()
                            && ui.button(tr(&self.config, "launch_options_menu")).clicked()
                        {
                            let g = &self.store.apps[idx];
                            self.tweak_args = g.launch_args.join("\n");
                            self.tweak_env = g.launch_env.join("\n");
                            self.tweak_popup = Some(idx);
                            auth::log_line("launch options popup opened");
                            // The open menu sits above the popup layer and
                            // swallows its clicks — close it when opening.
                            ui.close_menu();
                        }
                        #[cfg(not(target_os = "windows"))]
                        if !g.install_dir.is_empty()
                            && ui.button(tr(&self.config, "wine_settings")).clicked()
                        {
                            self.run_winecfg(idx);
                            ui.close_menu();
                        }
                        if !g.install_dir.is_empty()
                            && ui.button(tr(&self.config, "reset_prefix")).clicked()
                        {
                            self.reset_prefix(idx);
                            ui.close_menu();
                        }
                    });
                    ui.add_enabled_ui(!self.session.token.is_empty(), |ui| {
                        if ui.button(tr(&self.config, "check_updates")).clicked() {
                            // Pull the branch list (public + beta) fresh and
                            // report what newer build the store hands out.
                            // list_channels sets its own status; claiming a
                            // check here when it refused ("channels already
                            // loading") was a lie.
                            self.list_channels();
                            ui.close_menu();
                        }
                        if ui.button(tr(&self.config, "show_versions")).clicked() {
                            self.list_channels();
                            ui.close_menu();
                        }
                    });
                    let melon_ready = self
                        .store
                        .apps
                        .get(idx)
                        .map(|g| !g.install_dir.is_empty() && !g.partial)
                        .unwrap_or(false)
                        && !self.install_inflight
                        && !self.dl_busy(idx);
                    ui.add_enabled_ui(melon_ready, |ui| {
                        if ui.button(tr(&self.config, "install_melon")).clicked() {
                        // Per-game installer: versions list lives in a popup,
                        // not in Settings.
                        if self.melon_releases.is_empty() {
                            self.load_releases("LavaGang/MelonLoader");
                        }
                        self.melon_popup = Some(idx);
                        ui.close_menu();
                        }
                    });
                    #[cfg(not(target_os = "windows"))]
                    if ui.button(tr(&self.config, "proton")).clicked() {
                        // A nested menu drew its popup right over the entry it
                        // came from, hiding that row. A centered popup window
                        // keeps the ⋯ menu readable while a build is picked.
                        self.game_proton_popup = Some(idx);
                        ui.close_menu();
                    }
                });
                }
            });
        });
        ui.horizontal(|ui| {
            let covers = self.data_root().join("covers");
            let key = if g.meta_app_id.is_empty() {
                g.app_id.clone()
            } else {
                g.meta_app_id.clone()
            };
            if let Some(tex) =
                cover_texture(ctx, &covers, &key, &mut self.tex_cache).filter(|_| !self.covers_off)
            {
                let rect = ui
                    .allocate_response(egui::vec2(220.0, 293.0), egui::Sense::hover())
                    .rect;
                ui.painter().image(
                    tex.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                let rect = ui
                    .allocate_response(egui::vec2(220.0, 293.0), egui::Sense::hover())
                    .rect;
                ui.painter()
                    .rect_filled(rect, 6.0, egui::Color32::from_gray(40));
            }
            ui.vertical(|ui| {
                // Info first: size, last played, blurb. Install/play controls
                // live below; ids and versions live in the ⋯ menu.
                if g.partial {
                    ui.label(tr(&self.config, "card_partial"));
                } else if !installed {
                    ui.label(tr(&self.config, "card_not_installed"));
                }
                if installed && !g.install_dir.is_empty() {
                    let dir = g.install_dir.clone();
                    if let Some(bytes) = self.disk_sizes.get(&dir).copied() {
                        ui.label(i18n::fill(
                            &tr(&self.config, "disk_size_x"),
                            &[("size", &human_size(bytes))],
                        ));
                    } else {
                        if self.disk_pending.as_deref() != Some(dir.as_str()) {
                            self.disk_pending = Some(dir.clone());
                            let slot = self.disk_slot.clone();
                            let repaint = self.ctx.clone();
                            // An 11 GB folder takes seconds; never walk on UI.
                            std::thread::spawn(move || {
                                let bytes = dir_bytes(&PathBuf::from(&dir));
                                *slot.lock().unwrap() = Some((dir, bytes));
                                if let Some(c) = repaint {
                                    c.request_repaint();
                                }
                            });
                        }
                        ui.label(tr(&self.config, "measuring_size"));
                    }
                }
                match played_days(g.last_played, now_unix()) {
                    None => ui.label(tr(&self.config, "never_played")),
                    Some(0) => ui.label(tr(&self.config, "played_today")),
                    Some(1) => ui.label(tr(&self.config, "played_yesterday")),
                    Some(n) => ui.label(i18n::fill(
                        &tr(&self.config, "played_days"),
                        &[("n", &n.to_string())],
                    )),
                };
                if !g.description.is_empty() {
                    ui.label(g.description.clone());
                }
                if g.meta_app_id.is_empty() {
                    ui.label(tr(&self.config, "card_folder_scan"));
                }
                ui.label(tr(&self.config, "achievements_note"));
                if let Some(desc) = self.update_available() {
                    ui.colored_label(
                        egui::Color32::from_rgb(240, 200, 120),
                        format!("newer version out: {desc}"),
                    );
                }
            });
        });
        ui.separator();
        // Controls below the fold: Play/Reinstall and the release channel.
        ui.horizontal(|ui| {
            if self.running_flag.load(std::sync::atomic::Ordering::Relaxed) {
                // While the game lives, the button reads Running and
                // a click asks for confirmation before killing it.
                if ui
                    .add(
                        egui::Button::new(tr(&self.config, "running"))
                            .min_size(egui::vec2(120.0, 34.0)),
                    )
                    .clicked()
                {
                    self.stop_ask = Some(true);
                }
            } else {
                ui.add_enabled_ui(installed && !(self.dl_busy(idx)), |ui| {
                    if ui
                        .add(
                            egui::Button::new(tr(&self.config, "play"))
                                .min_size(egui::vec2(120.0, 34.0)),
                        )
                        .clicked()
                    {
                        self.launch();
                    }
                });
            }
            ui.add_enabled_ui(self.installable(idx), |ui| {
                if ui
                    .add(
                        egui::Button::new(if installed { "Reinstall" } else { "Install" })
                            .min_size(egui::vec2(120.0, 34.0)),
                    )
                    .clicked()
                {
                    self.open_install();
                }
            });
        });
        if !g.branches.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(tr(&self.config, "channel_label"));
                for (i, b) in g.branches.iter().enumerate() {
                    let label = format!("{} {}", b.name, b.version);
                    if ui.radio(i == self.selected_branch, &label).clicked() {
                        self.selected_branch = i;
                    }
                }
            });
        }
    }
}

/// Cover texture, decoded once and cached. Covers are resized down to card
/// size before upload so the GPU texture stays small (~260 KB each).
fn cover_texture(
    ctx: &egui::Context,
    dir: &Path,
    key: &str,
    cache: &mut std::collections::HashMap<String, egui::TextureHandle>,
) -> Option<egui::TextureHandle> {
    if let Some(t) = cache.get(key) {
        return Some(t.clone());
    }
    let path = dir.join(format!("{key}.img"));
    let bytes = std::fs::read(&path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let small = img.resize_to_fill(220, 293, image::imageops::FilterType::Lanczos3);
    let px = small.to_rgba8().into_raw();
    let w = small.width() as usize;
    let h = small.height() as usize;
    let ci = egui::ColorImage::from_rgba_unmultiplied([w, h], &px);
    let tex = ctx.load_texture(format!("cover-{key}"), ci, egui::TextureOptions::default());
    cache.insert(key.to_string(), tex.clone());
    Some(tex)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }
        apply_theme(ctx, &self.config.theme);
        self.poll_running_liveness();
        self.poll_launch();
        // egui idles without input events; background redeem/fetch results
        // and session.json changes must still be picked up, so keep a slow
        // heartbeat repaint going.
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
        if self.token_snapshot != self.session.token {
            self.token_snapshot = self.session.token.clone();
            self.token_changed_at = Some(std::time::Instant::now());
        }
        self.refresh_session();
        self.poll_redeem();
        if let Some((dir, bytes)) = self.disk_slot.lock().unwrap().take() {
            self.disk_sizes.insert(dir, bytes);
            self.disk_pending = None;
        }
        self.poll_fetch();
        self.poll_probe();
        self.poll_size();
        self.poll_download();
        self.poll_releases();
        // Drop textures for games that left the library (rescan purge, account
        // switch, renamed entries): orphan GPU textures used to pile up forever.
        let live: std::collections::HashSet<String> = self
            .store
            .apps
            .iter()
            .map(|g| {
                if g.meta_app_id.is_empty() {
                    g.app_id.clone()
                } else {
                    g.meta_app_id.clone()
                }
            })
            .collect();
        self.tex_cache.retain(|k, _| live.contains(k));
        self.poll_install();
        self.poll_channels();
        self.poll_check();
        self.auto_fetch_owned();
        let signed = !self.session.token.is_empty();
        // The green marker must die with the token: verified_account alone
        // used to keep "signed in" showing after Clear session deleted the
        // bearer, and an in-flight probe could re-set it for the old token.
        let verified = signed && self.verified_account.is_some();
        let owned = self.store.apps.iter().filter(|g| g.entitled).count();
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (marker_text, tone) = marker(signed, verified, self.probe_inflight);
                ui.horizontal(|ui| {
                    state_icon(ui, tone);
                    let marker = egui::RichText::new(marker_text).color(match tone {
                        "ok" => egui::Color32::from_rgb(90, 170, 90),
                        "warn" => egui::Color32::from_rgb(210, 170, 80),
                        _ => egui::Color32::from_rgb(150, 150, 150),
                    });
                    ui.label(marker);
                    if signed {
                        ui.label(format!("{owned} owned games"));
                    }
                });
                // The download line moved to the top of the page; the bottom
                // stays a clean account + status strip.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let shown = if self.status.is_empty() {
                        "ready".to_string()
                    } else {
                        // A long error must not run under the marker group on
                        // the left: clip it to the space still free here.
                        fit_text(&self.status, ui.available_width())
                    };
                    match status_tone(&self.status) {
                        // Guidance, not success: grey, never the green success color.
                        "guidance" => {
                            ui.colored_label(egui::Color32::from_rgb(170, 170, 170), &shown);
                        }
                        "error" => {
                            ui.colored_label(ui.style().visuals.error_fg_color, &shown);
                        }
                        _ => {
                            ui.colored_label(egui::Color32::from_rgb(140, 200, 140), &shown);
                        }
                    }
                });
            });
        });
        let settings = self.settings_page;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                // App logo beside the title, sized to the heading text.
                if self.logo_tex.is_none() {
                    if let Ok(img) =
                        image::load_from_memory(include_bytes!("../../assets/iris-logo.png"))
                    {
                        // Dimensions come from the asset, not hardcoded: the
                        // transparent rebuild shipped a 480x360 crop and the
                        // old [256, 256] assert killed startup on first frame.
                        let rgba = img.to_rgba8();
                        let w = rgba.width() as usize;
                        let h = rgba.height() as usize;
                        let ci = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba.into_raw());
                        self.logo_tex =
                            Some(ctx.load_texture("iris-logo", ci, egui::TextureOptions::default()));
                        self.logo_aspect = w as f32 / h as f32;
                    }
                }
                if let Some(tex) = &self.logo_tex {
                    let side = ui.style().text_styles[&egui::TextStyle::Heading].size * 1.25;
                    // Keep the asset's aspect ratio instead of stretching it
                    // into a square.
                    let aspect = self.logo_aspect;
                    ui.add(
                        egui::Image::new(tex)
                            .fit_to_exact_size(egui::vec2(side * aspect.max(1.0), side)),
                    );
                }
                ui.heading(if settings {
                    tr(&self.config, "settings")
                } else {
                    "Iris Game Launcher".to_string()
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let library_sel = !settings;
                    if ui
                        .selectable_label(library_sel, tr(&self.config, "library"))
                        .clicked()
                    {
                        self.settings_page = false;
                    }
                    if ui.selectable_label(settings, tr(&self.config, "settings")).clicked() {
                        self.settings_page = true;
                    }
                });
            });
            // Downloads watch at the top: the bottom bar was getting cluttered.
            if self.dl_inflight {
                let (done, total, bytes, tot) = *self.dl_progress.lock().unwrap();
                let frac = if tot > 0 {
                    (bytes as f64 / tot as f64) as f32
                } else {
                    0.0
                }
                ;
                ui.horizontal(|ui| {
                    let name = self
                        .store
                        .apps
                        .iter()
                        .find(|e| e.app_id == self.dl_key)
                        .map(|e| e.name.clone())
                        .unwrap_or_else(|| self.dl_label.clone());
                    ui.label(i18n::fill(
                        &i18n::text(i18n::parse(&self.config.lang), "downloading_x"),
                        &[("name", &name)],
                    ));
                    // The percent text inside the bar collided with the
                    // animated strip; keep counts beside the bar instead.
                    ui.label(format!(
                        "{:.0}% · {done}/{total} files · {} of {}",
                        frac * 100.0,
                        human_bytes(bytes),
                        human_bytes(tot)
                    ));
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .text(String::new())
                            .animate(true)
                            .desired_width(ui.available_width().min(260.0)),
                    );
                    if ui.button(tr(&self.config, "cancel_download")).clicked() {
                        self.dl_stop.store(true, std::sync::atomic::Ordering::Relaxed);
                        self.status =
                            "cancel requested — the download stops after the current part".into();
                    }
                });
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if settings {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(tr(&self.config, "settings_appearance"));
                        ui.horizontal(|ui| {
                            // One dropdown per selector instead of a row of
                            // buttons: the current value is the button label.
                            ui.label(tr(&self.config, "theme"));
                            let cur_theme = tr(&self.config, &format!("theme_{}", self.config.theme));
                            ui.menu_button(cur_theme, |ui| {
                                for th in ["dark", "light", "solarized"] {
                                    if ui.button(tr(&self.config, &format!("theme_{th}"))).clicked() {
                                        self.config.theme = th.to_string();
                                        self.save_config();
                                        ui.close_menu();
                                    }
                                }
                            });
                            ui.label(tr(&self.config, "language"));
                            let cur_lang = tr(&self.config, &format!("lang_{}", self.config.lang));
                            ui.menu_button(cur_lang, |ui| {
                                for code in ["en", "pl"] {
                                    if ui.button(tr(&self.config, &format!("lang_{code}"))).clicked() {
                                        self.config.lang = code.to_string();
                                        self.save_config();
                                        ui.close_menu();
                                    }
                                }
                            });
                        });
                    });
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(tr(&self.config, "settings_folders"));
                        ui.label("Prefix dir:");
                        ui.horizontal(|ui| {
                            if ui.add(
                            egui::TextEdit::singleline(&mut self.config.prefix_dir)
                                .desired_width(ui.available_width() - 90.0),
                        )
                        .changed()
                        {
                                self.save_config();
                            }
                            if ui.button("Pick folder").clicked() {
                                let start = if self.config.prefix_dir.is_empty() {
                                    std::path::PathBuf::from("/")
                                } else {
                                    PathBuf::from(self.config.prefix_dir.trim())
                                };
                                self.dir_popup = Some((start, DirTarget::PrefixDir));
                            }
                        });
                        #[cfg(not(target_os = "windows"))]
                        {
                            ui.label("Wine binary (used when no Proton picked):");
                            if ui.add(
                            egui::TextEdit::singleline(&mut self.config.wine_binary)
                                .desired_width(ui.available_width()),
                        )
                        .changed()
                        {
                                self.save_config();
                            }
                        }
                        ui.label("Download dir (store downloads unpack here):");
                        ui.horizontal(|ui| {
                            if ui.add(
                            egui::TextEdit::singleline(&mut self.config.download_dir)
                                .desired_width(ui.available_width() - 90.0),
                        )
                        .changed()
                        {
                                self.save_config();
                            }
                            if ui.button("Pick folder").clicked() {
                                let start = if self.config.download_dir.is_empty() {
                                    std::path::PathBuf::from("/")
                                } else {
                                    PathBuf::from(self.config.download_dir.trim())
                                };
                                self.dir_popup = Some((start, DirTarget::DownloadDir));
                            }
                        });
                        if ui.button(tr(&self.config, "import_game")).clicked() {
                            let start = if !self.config.download_dir.is_empty() {
                                PathBuf::from(self.config.download_dir.trim())
                            } else {
                                PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
                            };
                            self.dir_popup = Some((start, DirTarget::Import));
                        }
                        if ui.button("Open download dir").clicked() {
                            open_folder(&PathBuf::from(&self.config.download_dir));
                        }
                        if ui.button("Open log file").clicked() {
                            if let Some(p) = auth::log_path() {
                                open_folder(Path::new(&p));
                                self.status = format!("log: {p}");
                            }
                        }
                    });
                    #[cfg(not(target_os = "windows"))]
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(tr(&self.config, "settings_proton"));
                        ui.label(tr(&self.config, "proton_dirs_hint"));
                            let joined = self.config.proton_dirs.join("\n");
                            let mut text = joined.clone();
                            if ui
                                .add(
                                    egui::TextEdit::multiline(&mut text)
                                        .desired_width(ui.available_width()),
                                )
                                .changed()
                            {
                                self.config.proton_dirs = text
                                    .lines()
                                    .map(|l| l.trim().to_string())
                                    .filter(|l| !l.is_empty())
                                    .collect();
                                self.save_config();
                            }
                            let found = detect_protons(&self.config.proton_dirs);
                            if found.is_empty() {
                                ui.label("no proton runscript found in those dirs");
                            }
                            // Dropdown instead of a radio wall; paths shown short.
                            let picked = if self.config.proton_selected.is_empty() {
                                "own wine".to_string()
                            } else {
                                short_path(&self.config.proton_selected)
                            };
                            egui::ComboBox::from_id_salt("proton_runtime")
                                .selected_text(picked)
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_value(
                                            &mut self.config.proton_selected,
                                            String::new(),
                                            "own wine",
                                        )
                                        .changed()
                                    {
                                        self.save_config();
                                    }
                                    for p in found.clone() {
                                        if ui
                                            .selectable_value(
                                                &mut self.config.proton_selected,
                                                p.clone(),
                                                short_path(&p),
                                            )
                                            .changed()
                                        {
                                            self.save_config();
                                        }
                                    }
                                });
                            ui.label(format!(
                                "launcher runtime: {}",
                                if self.config.proton_selected.is_empty() {
                                    "wine"
                                } else {
                                    "proton"
                                }
                            ));
                    });
                        #[cfg(not(target_os = "windows"))]
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.label(tr(&self.config, "proton_ge_hint"));
                            if ui.button("Choose a Proton version").clicked() {
                                self.proton_popup = true;
                                if self.proton_releases.is_empty() {
                                    self.load_releases("GloriousEggroll/proton-ge-custom");
                                }
                            }
                        });
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(tr(&self.config, "settings_data"));
                        ui.label(format!("data root: {}", self.store_path.parent().unwrap_or(Path::new(".")).display()));
                        if ui.button("Save config").clicked() {
                            match serde_json::to_string_pretty(&self.config) {
                                Ok(s) => {
                                    // "config saved" must mean the bytes landed;
                                    // .ok() hid write failures behind a green lie.
                                    match std::fs::write(&self.config_path, s) {
                                        Ok(()) => self.status = "config saved".into(),
                                        Err(e) => self.status = format!("could not save config: {e}"),
                                    }
                                }
                                Err(e) => self.status = format!("config error: {e}"),
                            }
                        }
                    });
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(tr(&self.config, "login_header"));
                        ui.label(tr(&self.config, "login_help_1"));
                        ui.horizontal(|ui| {
                            if ui.button(tr(&self.config, "login_btn")).clicked() {
                                self.login_in_browser();
                            }
                            #[cfg(target_os = "linux")]
                            {
                                let owner = auth::handler_owner();
                                let label = if owner == "other" {
                                    tr(&self.config, "handler_repoint")
                                } else {
                                    tr(&self.config, "handler_set")
                                };
                                if owner != "launcher" && ui.button(label).clicked() {
                                    match auth::install_oculus_handler() {
                                        Ok(p) => self.status = format!("headset link handler: {p}"),
                                        Err(e) => {
                                            self.status = format!("handler install failed: {e}")
                                        }
                                    }
                                }
                            }
                            #[cfg(target_os = "windows")]
                            {
                                let owner = auth::handler_owner();
                                let label = if owner == "launcher" {
                                    "Give the headset link back to the vendor app"
                                } else {
                                    "Take the headset link (launcher)"
                                };
                                if ui.button(label).clicked() {
                                    let res = if owner == "launcher" {
                                        auth::uninstall_oculus_handler()
                                    } else {
                                        auth::install_oculus_handler()
                                    };
                                    match res {
                                        Ok(p) => self.status = format!("headset link handler: {p}"),
                                        Err(e) => {
                                            self.status = format!("handler change failed: {e}")
                                        }
                                    }
                                }
                            }
                        });
                        #[cfg(any(target_os = "linux", target_os = "windows"))]
                        ui.label(format!("headset link handler: {}", auth::handler_status()));
                        ui.label(tr(&self.config, "login_help_2"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.callback_url)
                                .hint_text("oculus://link/… the link the browser opened")
                                .desired_width(320.0),
                        );
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.session.native_sso_manual)
                                .hint_text("paste a login token here only if Apply says it did not match")
                                .desired_width(320.0),
                        );
                        if resp.changed() {
                            // Handler processes read this field from
                            // session.json, so an unsaved paste could never
                            // rescue a hash mismatch.
                            self.save_session();
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Apply").clicked() {
                                self.redeem_callback();
                            }
                            if ui.button(tr(&self.config, "check_login")).clicked() {
                                // Ask the store whether this token still works.
                                // The answer lands in the status line and the
                                // bottom marker (green only when confirmed).
                                if self.session.token.is_empty() {
                                    self.status = "paste a login token first".into();
                                } else {
                                    self.start_probe();
                                }
                            }
                            if ui.button(tr(&self.config, "forget_token")).clicked() {
                                self.clear_session();
                            }
                        });
                        ui.label("Apply turns the browser login link into a token. Check login asks the store if the token still works. Forget deletes the token, the verified account and the login params from disk.");
                        ui.label(format!(
                            "token: {}  browser login params: {}",
                            if self.session.token.is_empty() { "none" } else { "present" },
                            if self.session.frl_request.is_empty() {
                                "none"
                            } else if self.session.callback_attempted {
                                "stored, last attempt failed"
                            } else {
                                "stored, not applied"
                            }
                        ));
                        ui.label(format!("token kind: {}", token_kind(&self.session.token)));
                        // Persist on change like the store-id field: closing the
                        // app without saving used to discard the paste. The token
                        // is masked by default; reveal shows what got pasted.
                        ui.horizontal(|ui| {
                            let mut edit =
                                egui::TextEdit::singleline(&mut self.session.token)
                                    .desired_width(300.0);
                            if !self.token_revealed {
                                edit = edit.password(true);
                            }
                            if ui.add(edit).changed() {
                                self.save_session();
                            }
                            if ui
                                .button(if self.token_revealed { "hide" } else { "reveal" })
                                .clicked()
                            {
                                self.token_revealed = !self.token_revealed;
                            }
                        });
                        ui.label("3. ⟳ refresh library on the main screen fills the list from your account. Open a game, then press Install.");
                        ui.horizontal(|ui| {
                            let name = self
                                .store
                                .apps
                                .get(self.selected)
                                .map(|g| g.name.clone())
                                .unwrap_or_else(|| "nothing selected".into());
                            ui.label(format!("Store id of selected ({name}):"));
                            if let Some(g) = self.store.apps.get_mut(self.selected) {
                                // Persist the edit right away: closing Settings without
                                // another action used to drop the typed id silently.
                                if ui.text_edit_singleline(&mut g.meta_app_id).changed() {
                                    self.save_store();
                                }
                            }
                        });
                        if let Some(id) = self.verified_account {
                            ui.label(format!("account id: {id} (the store only reports this number)"));
                        }
                    });
                    #[cfg(target_os = "windows")]
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(tr(&self.config, "revive_section"));
                            ui.horizontal(|ui| {
                                if ui.button(tr(&self.config, "revive_download")).clicked() {
                                    self.download_revive_setup();
                                }
                                if ui.button(tr(&self.config, "revive_run_installer")).clicked() {
                                    self.run_revive_installer();
                                }
                                if ui.button(tr(&self.config, "revive_find")).clicked() {
                                    self.find_revive_layer();
                                }
                            });
                            ui.label(tr(&self.config, "revive_meta_hint"));
                            ui.horizontal(|ui| {
                                if ui.button(tr(&self.config, "revive_meta_takeover")).clicked() {
                                    self.take_meta_runtime();
                                }
                                if ui.button(tr(&self.config, "revive_meta_revert")).clicked() {
                                    self.restore_meta_runtime();
                                }
                            });
                            ui.label(tr(&self.config, "revive_dll_hint"));
                            let mut text = self.config.revive_dll.clone();
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut text)
                                        .hint_text("LibOVRRT64_1.dll path")
                                        .desired_width(320.0),
                                )
                                .changed()
                            {
                                self.config.revive_dll = text.trim().to_string();
                                self.save_config();
                            }
                            ui.label(tr(&self.config, "ovr_server_hint"));
                            let mut stext = self.config.ovr_server.clone();
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut stext)
                                        .hint_text("OVRServer_x64.exe path (optional)")
                                        .desired_width(320.0),
                                )
                                .changed()
                            {
                                self.config.ovr_server = stext.trim().to_string();
                                self.save_config();
                            }
                        });
                } else if let Some(idx) = self.open_game.filter(|i| self.store.apps.get(*i).is_some()) {
                    self.game_page(ctx, ui, idx);
                } else {
                    ui.horizontal(|ui| {
                        let signed_in = !self.session.token.is_empty();
                        if ui.button("Rescan folders").clicked() {
                            self.rescan_dirs();
                        }
                        ui.add_enabled_ui(signed_in, |ui| {
                            if ui.button("⟳ refresh library").clicked() {
                                self.fetch_owned();
                            }
                        });
                        if self.fetch_inflight {
                            // Truthful: a library already on screen is not
                            // "loading" — a refresh in flight is a refresh.
                            if self.store.apps.is_empty() {
                                ui.label("loading library…");
                            } else {
                                ui.label("refreshing…");
                            }
                        }
                    });
                    let mut order: Vec<usize> = (0..self.store.apps.len()).collect();
                    order.sort_by(|a, b| {
                        self.store.apps[*b]
                            .last_played
                            .cmp(&self.store.apps[*a].last_played)
                            .then_with(|| self.store.apps[*a].name.cmp(&self.store.apps[*b].name))
                    });
                    let covers = self.data_root().join("covers");
                    if self.store.apps.is_empty() {
                        ui.label("library empty — press ⟳ refresh library after signing in, or Rescan folders for local installs");
                    }
                    // Responsive vertical cards: column count follows the
                    // panel width, cards fill it, no cropped dead space.
                    let avail_w = ui.available_width();
                    let gap = 10.0_f32;
                    let cols = ((avail_w + gap) / (170.0 + gap)).floor().max(1.0) as usize;
                    let card_w = ((avail_w - gap * (cols - 1) as f32) / cols as f32)
                        .clamp(90.0_f32.min(avail_w), avail_w);
                    let card_h = card_w * 4.0 / 3.0 + 30.0;
                    for chunk in order.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for &idx in chunk {
                                let key = {
                                    let g = &self.store.apps[idx];
                                    if g.meta_app_id.is_empty() {
                                        g.app_id.clone()
                                    } else {
                                        g.meta_app_id.clone()
                                    }
                                };
                                let name = self.store.apps[idx].name.clone();
                                let resp =
                                    ui.allocate_response(egui::vec2(card_w, card_h), egui::Sense::click());
                                let rect = resp.rect;
                                let cover_rect = egui::Rect::from_min_max(
                                    rect.min,
                                    egui::pos2(rect.max.x, rect.max.y - 26.0),
                                );
                                if let Some(tex) =
                                    cover_texture(ctx, &covers, &key, &mut self.tex_cache).filter(|_| !self.covers_off)
                                {
                                    let uv = egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    );
                                    ui.painter()
                                        .image(tex.id(), cover_rect, uv, egui::Color32::WHITE);
                                } else {
                                    ui.painter().rect_filled(
                                        cover_rect,
                                        6.0,
                                        egui::Color32::from_gray(40),
                                    );
                                }
                                if self.selected == idx {
                                    ui.painter().rect_stroke(
                                        rect,
                                        6.0,
                                        egui::Stroke::new(2.0_f32, egui::Color32::from_gray(200)),
                                    );
                                }
                                // The cover is the whole card; the painted line
                                // is the name only. Install state lives on the
                                // game page and in the ⋯ menu, not under the title.
                                let short: String = name.chars().take(26).collect();
                                let line = short.clone();
                                ui.painter().text(
                                    egui::pos2(rect.min.x + 4.0, rect.max.y - 4.0),
                                    egui::Align2::LEFT_BOTTOM,
                                    &line,
                                    egui::FontId::default(),
                                    egui::Color32::from_gray(210),
                                );
                                if resp.clicked() {
                                    self.selected = idx;
                                    self.selected_branch = 0;
                                    self.open_game = Some(idx);
                                }
                            }
                        });
                    }
                }
            });
        });
        // Stop dialog: killing a game takes its whole Wine/Proton tree, so
        // the button asks first instead of stopping on the click.
        if self.stop_ask.is_some() {
            egui::Window::new(tr(&self.config, "stop_title"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.label(tr(&self.config, "stop_question"));
                    ui.horizontal(|ui| {
                        if ui.button(tr(&self.config, "stop_soft")).clicked() {
                            self.stop_ask = None;
                            self.stop_running(false);
                        }
                        if ui.button(tr(&self.config, "stop_hard")).clicked() {
                            self.stop_ask = None;
                            self.stop_running(true);
                        }
                        if ui.button(tr(&self.config, "stop_cancel")).clicked() {
                            self.stop_ask = None;
                        }
                    });
                });
        }
        // Folder-import mapping: an external folder may BE a store game;
        // picking its library entry fills id, cover and description instead
        // of leaving a second-class folder entry.
        if let Some((dir, sel)) = self.import_ask.clone() {
            let entries: Vec<(usize, String)> = self
                .store
                .apps
                .iter()
                .enumerate()
                .filter(|(_, g)| !g.meta_app_id.is_empty())
                .map(|(i, g)| (i, g.name.clone()))
                .collect();
            let mut chosen = sel;
            let mut apply = false;
            let mut cancel = false;
            egui::Window::new(tr(&self.config, "import_match_title"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if entries.is_empty() {
                        ui.label(tr(&self.config, "import_match_no_store"));
                    }
                    for (i, name) in &entries {
                        if ui.radio(chosen == Some(*i), name).clicked() {
                            chosen = Some(*i);
                        }
                    }
                    if ui
                        .radio(chosen.is_none(), tr(&self.config, "import_match_none"))
                        .clicked()
                    {
                        chosen = None;
                    }
                    ui.horizontal(|ui| {
                        if ui.button(tr(&self.config, "import_apply")).clicked() {
                            apply = true;
                        }
                        if ui.button(tr(&self.config, "close")).clicked() {
                            cancel = true;
                        }
                    });
                });
            if apply || cancel {
                self.import_ask = None;
                if apply {
                    match chosen {
                        Some(i) => self.attach_imported(i, &dir),
                        None => self.plain_import(&dir),
                    }
                }
            }
        }
        if let Some(idx) = self.tweak_popup {
            // The text fields live on the App, not a per-frame clone: a
            // clone from the entry overwrote whatever was being typed.
            let mut close = false;
            let name = self.store.apps[idx].name.clone();
            egui::Window::new(i18n::fill(
                &tr(&self.config, "launch_options_title"),
                &[("name", &name)],
            ))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            // Foreground layer: menus paint here too, and the popup is
            // shown last in that layer, so nothing above can eat its clicks.
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label(tr(&self.config, "tweak_hint"));
                ui.colored_label(
                    egui::Color32::from_gray(150),
                    tr(&self.config, "launch_args"),
                );
                ui.text_edit_multiline(&mut self.tweak_args);
                ui.colored_label(
                    egui::Color32::from_gray(150),
                    tr(&self.config, "launch_env"),
                );
                ui.text_edit_multiline(&mut self.tweak_env);
                // Each preset is a radio that adds or removes ONLY its
                // own entry. The old buttons rewrote the whole field, so
                // pressing one silently dropped the others.
                ui.label(tr(&self.config, "presets_label"));
                #[cfg_attr(target_os = "windows", allow(unused_mut))]
                let mut presets = vec![
                    // NVIDIA's winewayland driver is the known failure;
                    // forcing the X11 driver through XWayland is the fix
                    // when no X11 session is available.
                    ("XWayland driver", Preset::Dll("winewayland=d")),
                    ("Native dxgi", Preset::Dll("dxgi=n")),
                    // MelonLoader's version.dll proxy needs native-first
                    // lookup under Wine.
                    ("MelonLoader proxy", Preset::Dll("version=n,b")),
                    ("Unity DX11", Preset::Arg("-force-gfx-directx11")),
                    ("Unity GL core", Preset::Arg("-force-glcore")),
                    // Writes every shim call into iris-shim-trace.log
                    // next to the exe. Off by default: Bonelab polls the
                    // message queue thousands of times per session.
                    ("Shim trace log", Preset::Env("OVR_SHIM_LOG")),
                ];
                #[cfg(not(target_os = "windows"))]
                // Proton's OpenVR/OpenXR bridge needs the native runtime
                // root; vrserver must already be running when the game
                // asks, otherwise the bridge falls back to flat mode.
                presets.push(("SteamVR runtime", Preset::SteamVr));
                for (label, preset) in presets {
                    let (item, on, changed) = match preset {
                        Preset::Dll(item) => {
                            let env = env_lines(&self.tweak_env);
                            let on = has_dll_override(&env, item);
                            (item, on, false)
                        }
                        Preset::Arg(item) => {
                            let args = env_lines(&self.tweak_args);
                            let on = has_arg(&args, item);
                            (item, on, false)
                        }
                        Preset::Env(key) => {
                            let env = env_lines(&self.tweak_env);
                            let on = has_env_key(&env, key);
                            (key, on, false)
                        }
                        #[cfg(not(target_os = "windows"))]
                        Preset::SteamVr => {
                            let env = env_lines(&self.tweak_env);
                            let on = has_env_key(&env, "PROTON_VR_RUNTIME");
                            (label, on, false)
                        }
                    };
                    let _ = changed;
                    ui.horizontal(|ui| {
                        ui.label(label);
                        if ui.radio(on, tr(&self.config, "preset_on")).clicked() && !on {
                            self.apply_preset(idx, preset, item, true);
                        }
                        if ui.radio(!on, tr(&self.config, "preset_off")).clicked() && on {
                            self.apply_preset(idx, preset, item, false);
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button(tr(&self.config, "tweak_save")).clicked() {
                        if let Some(e) = self.store.apps.get_mut(idx) {
                            e.launch_args = self
                                .tweak_args
                                .lines()
                                .map(|l| l.trim().to_string())
                                .filter(|l| !l.is_empty())
                                .collect();
                            e.launch_env = self
                                .tweak_env
                                .lines()
                                .map(|l| l.trim().to_string())
                                .filter(|l| !l.is_empty())
                                .collect();
                        }
                        self.save_store();
                        self.status = i18n::fill(
                            &tr(&self.config, "launch_options_saved"),
                            &[("name", &self.store.apps[idx].name)],
                        );
                        close = true;
                    }
                    if ui.button(tr(&self.config, "tweak_clear")).clicked() {
                        self.tweak_args.clear();
                        self.tweak_env.clear();
                        if let Some(e) = self.store.apps.get_mut(idx) {
                            e.launch_args.clear();
                            e.launch_env.clear();
                            self.save_store();
                        }
                        self.status = i18n::fill(
                            &tr(&self.config, "launch_options_cleared"),
                            &[("name", &self.store.apps[idx].name)],
                        );
                    }
                    if ui.button(tr(&self.config, "cancel")).clicked() {
                        // Quit with the stored values untouched.
                        close = true;
                    }
                });
            });
            if close {
                self.tweak_popup = None;
            }
        }
        if let Some(idx) = self.custom_pick {
            let dir = self
                .store
                .apps
                .get(idx)
                .map(|g| PathBuf::from(&g.install_dir))
                .unwrap_or_default();
            let files = exe_files(&dir);
            egui::Window::new("Run another exe in this folder")
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if files.is_empty() {
                        ui.label("no runnable exe found in this folder");
                    } else {
                        ui.label(short_path(&dir.display().to_string()));
                        for (i, f) in files.iter().enumerate() {
                            if ui
                                .radio(i == 0, f.file_name().unwrap_or_default().to_string_lossy())
                                .clicked()
                            {
                                self.run_in_prefix(idx, f);
                                self.custom_pick = None;
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.custom_pick = None;
                    }
                });
        }
        if let Some(p) = self.install_prompt.clone() {
            let name = self
                .store
                .apps
                .get(p.app_idx)
                .map(|g| g.name.clone())
                .unwrap_or_default();
            egui::Window::new(format!("Install {name}"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("version: {}", p.channel));
                    if p.sizing {
                        ui.label("measuring the download size…");
                    } else {
                        ui.label(format!(
                            "needs {} in {} files",
                            human_bytes(p.bytes),
                            p.files
                        ));
                    }
                    ui.label("install folder:");
                    ui.horizontal(|ui| {
                        if let Some(pp) = self.install_prompt.as_mut() {
                            ui.add(egui::TextEdit::singleline(&mut pp.dir).desired_width(260.0));
                        }
                        if ui.button("Pick folder").clicked() {
                            let start = self
                                .install_prompt
                                .as_ref()
                                .map(|p| PathBuf::from(p.dir.trim()))
                                .unwrap_or_else(|| self.data_root().join("games"));
                            self.pick_dir(start, DirTarget::Install);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(
                            !p.sizing && !p.dir.trim().is_empty() && !self.session.token.is_empty(),
                            |ui| {
                                if ui.button("Install here").clicked() {
                                    self.confirm_install();
                                }
                            },
                        );
                        if ui.button("Cancel").clicked() {
                            self.install_prompt = None;
                        }
                    });
                });
        }
        // Folder chooser: egui has no native dialog, so this browses subfolders.
        if let Some((start, target)) = self.dir_popup.clone() {
            egui::Window::new("Choose a folder")
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!(
                        "inside: {}",
                        short_path(&start.display().to_string())
                    ));
                    let mut subs: Vec<String> = std::fs::read_dir(&start)
                        .map(|rd| {
                            rd.filter_map(|e| {
                                e.ok().and_then(|e| {
                                    (e.path().is_dir() && !e.path().is_symlink())
                                        .then(|| e.path().display().to_string())
                                })
                            })
                            .collect()
                        })
                        .unwrap_or_default();
                    subs.sort();
                    if subs.is_empty() {
                        ui.label("no subfolders here");
                    }
                    for e in subs.iter().take(30) {
                        let nm = Path::new(e)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if ui.button(format!("open {nm}")).clicked() {
                            self.dir_popup = Some((PathBuf::from(e), target));
                        }
                    }
                    ui.separator();
                    ui.label(format!(
                        "use this: {}",
                        short_path(&start.display().to_string())
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Use this folder").clicked() {
                            let text = start.display().to_string();
                            match target {
                                DirTarget::Install => {
                                    if let Some(pp) = &mut self.install_prompt {
                                        pp.dir = text;
                                    }
                                }
                                DirTarget::DownloadDir => {
                                    self.config.download_dir = text;
                                    self.save_config();
                                }
                                DirTarget::PrefixDir => {
                                    self.config.prefix_dir = text;
                                    self.save_config();
                                }
                                DirTarget::Import => {
                                    self.import_folder(PathBuf::from(&text));
                                }
                            }
                            self.dir_popup = None;
                        }
                        if start.parent().is_some() && ui.button("Up one level").clicked() {
                            if let Some(p) = start.parent() {
                                self.dir_popup = Some((p.to_path_buf(), target));
                            }
                        }
                        if ui.button("Close").clicked() {
                            self.dir_popup = None;
                        }
                    });
                });
        }
        if let Some(idx) = self.uninstall_ask {
            let name = self
                .store
                .apps
                .get(idx)
                .map(|g| g.name.clone())
                .unwrap_or_default();
            egui::Window::new(format!("Uninstall {name}"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                ui.label("This deletes the game folder from disk. The library entry stays, so you can install again later.");
                ui.horizontal(|ui| {
                    // A download writing this folder must not race remove_dir_all.
                    ui.add_enabled_ui(!(self.dl_busy(idx)), |ui| {
                        if ui.button("Remove files").clicked() {
                            self.uninstall(idx);
                            self.uninstall_ask = None;
                            self.open_game = None;
                        }
                    });
                    if ui.button("Cancel").clicked() {
                        self.uninstall_ask = None;
                    }
                });
            });
        }
        // Per-game Proton picker: which build launches this game.
        #[cfg(not(target_os = "windows"))]
        if let Some(idx) = self.game_proton_popup {
            let name = self
                .store
                .apps
                .get(idx)
                .map(|g| g.name.clone())
                .unwrap_or_default();
            let picked = self
                .store
                .apps
                .get(idx)
                .map(|g| g.proton.clone())
                .unwrap_or_default();
            egui::Window::new(tr(&self.config, "proton"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.label(i18n::fill(
                        &tr(&self.config, "proton_for_x"),
                        &[("name", &name)],
                    ));
                    let protons = detect_protons(&self.config.proton_dirs);
                    if protons.is_empty() {
                        ui.label(tr(&self.config, "proton_none"));
                    } else {
                        if ui
                            .radio(picked.is_empty(), tr(&self.config, "proton_global"))
                            .clicked()
                        {
                            if let Some(e) = self.store.apps.get_mut(idx) {
                                e.proton.clear();
                                self.save_store();
                            }
                        }
                        for pr in protons {
                            // Scripts are named `proton` inside their version
                            // folder, so the folder name is what tells them apart.
                            let label = std::path::Path::new(&pr)
                                .parent()
                                .and_then(|d| d.file_name())
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| pr.clone());
                            if ui.radio(picked == pr, &label).clicked() {
                                if let Some(e) = self.store.apps.get_mut(idx) {
                                    e.proton = pr.clone();
                                    self.save_store();
                                }
                            }
                        }
                    }
                    if ui.button(tr(&self.config, "close")).clicked() {
                        #[cfg(not(target_os = "windows"))]
                        {
                            self.game_proton_popup = None;
                        }
                    }
                });
        }
        if let Some(idx) = self.melon_popup {
            let name = self
                .store
                .apps
                .get(idx)
                .map(|g| g.name.clone())
                .unwrap_or_default();
            let installed = self
                .store
                .apps
                .get(idx)
                .map(|g| !g.install_dir.is_empty() && !g.partial)
                .unwrap_or(false);
            egui::Window::new(format!("MelonLoader for {name}"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if !installed {
                        ui.label("this game is not installed yet — install it first");
                    }
                    if self.melon_releases.is_empty() {
                        ui.label("loading the version list from GitHub…");
                    } else {
                        for (i, (tag, _, _)) in self.melon_releases.iter().enumerate() {
                            if ui.radio(i == self.melon_pick, tag).clicked() {
                                self.melon_pick = i;
                            }
                        }
                        ui.add_enabled_ui(
                            installed && !self.install_inflight && !(self.dl_busy(idx)),
                            |ui| {
                                if ui.button("Install into this game").clicked() {
                                    self.install_melon(idx);
                                    self.melon_popup = None;
                                }
                            },
                        );
                    }
                    if ui.button("Close").clicked() {
                        self.melon_popup = None;
                    }
                });
        }
        #[cfg(not(target_os = "windows"))]
        if self.proton_popup {
            egui::Window::new("Proton version")
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if self.proton_releases.is_empty() {
                        ui.label("loading the release list from GitHub…");
                    } else {
                        let picked = self
                            .proton_releases
                            .get(self.proton_pick)
                            .map(|(tag, asset, _)| format!("{tag} ({asset})"))
                            .unwrap_or_else(|| "pick a version".into());
                        egui::ComboBox::from_id_salt("proton_ge_release")
                            .selected_text(picked)
                            .show_ui(ui, |ui| {
                                for (i, (tag, asset, _url)) in
                                    self.proton_releases.iter().enumerate()
                                {
                                    if ui
                                        .selectable_value(
                                            &mut self.proton_pick,
                                            i,
                                            format!("{tag} ({asset})"),
                                        )
                                        .changed()
                                    {
                                        self.proton_pick = i;
                                    }
                                }
                            });
                        ui.horizontal(|ui| {
                            // No Steam on this machine: the Steam radio would
                            // create a phantom folder Steam never reads.
                            ui.add_enabled_ui(steam_present(), |ui| {
                                ui.radio_value(&mut self.proton_to_steam, true, "install for Steam too");
                            });
                            ui.radio_value(&mut self.proton_to_steam, false, "install only for the launcher");
                        });
                        if !steam_present() {
                            self.proton_to_steam = false;
                            ui.label("no Steam install found here, so only the launcher folder is offered");
                        } else {
                            ui.label("Steam picks tools up from compatibilitytools.d by itself; the launcher folder is only needed for games launched here.");
                        }
                        ui.add_enabled_ui(!self.install_inflight, |ui| {
                            if ui.button("Download chosen Proton").clicked() {
                                self.install_proton();
                                self.proton_popup = false;
                            }
                        });
                    }
                    if ui.button("Close").clicked() {
                        self.proton_popup = false;
                    }
                });
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|a| a == "rescan").unwrap_or(false) {
        let mut app = App::new(false);
        app.rescan_dirs();
        println!("{}", app.status);
        return Ok(());
    }
    if args.get(1).map(|a| a == "--download").unwrap_or(false) {
        let Some(id) = args.get(2) else {
            println!("usage: --download <meta app id>");
            return Ok(());
        };
        let mut app = App::new(false);
        let dest_override = args.get(3).map(|a| a.as_str());
        let msg = app.download_cli(id, dest_override);
        println!("{msg}");
        if msg.contains("failed") || msg.contains("no exe") {
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.get(1).map(|a| a == "--reset-prefix").unwrap_or(false) {
        // Delete the per-game prefixes under the configured prefix root. The
        // library store is untouched; games just need Wine re-initialising.
        let app = App::new(false);
        let root = game_prefix_root(&app.config.prefix_dir);
        if root == std::path::Path::new("/") || root == std::path::Path::new("") {
            println!("refusing to wipe the filesystem root as a prefix dir");
            std::process::exit(1);
        }
        let mut removed = 0usize;
        if let Ok(rd) = std::fs::read_dir(&root) {
            for sub in rd.flatten() {
                if sub.path().is_dir() && std::fs::remove_dir_all(sub.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        println!("{removed} game prefixes removed under {}", root.display());
        return Ok(());
    }
    if args.get(1).map(|a| a == "install-handler").unwrap_or(false) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        match auth::install_oculus_handler() {
            Ok(p) => println!("headset link handler installed: {p}"),
            Err(e) => eprintln!("handler install failed: {e}"),
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        eprintln!("no headset link handler on this platform");
        return Ok(());
    }
    if args
        .get(1)
        .map(|a| a == "uninstall-handler")
        .unwrap_or(false)
    {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        match auth::uninstall_oculus_handler() {
            Ok(p) => println!("{p}"),
            Err(e) => eprintln!("handler uninstall failed: {e}"),
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        eprintln!("no headset link handler on this platform");
        return Ok(());
    }
    if args.get(1).map(|a| a == "login-url").unwrap_or(false) {
        match auth::start_login() {
            Ok((url, _, _)) => println!("{url}"),
            Err(e) => eprintln!("login start failed: {e}"),
        }
        return Ok(());
    }
    if args.get(1).map(|a| a == "--url").unwrap_or(false) {
        let Some(url) = args.get(2) else {
            eprintln!("missing callback url");
            return Ok(());
        };
        let mut app = App::new(false);
        match auth::callback_params(url) {
            Some((token_param, blob)) => {
                app.session.frl_request = token_param.clone();
                app.session.frl_blob = blob.clone();
                auth::log_line(&format!(
                    "callback via --url token={} blob={blob}",
                    auth::log_peek(&token_param),
                    blob = blob
                ));
                // request_token must be the native SSO token whose sha256[..16]
                // equals the URL token param, same gate as the GUI redeem.
                let mut request_token: Option<String> = None;
                if !app.session.native_sso_token.is_empty()
                    && auth::callback_token_matches(&app.session.native_sso_token, &token_param)
                {
                    request_token = Some(app.session.native_sso_token.clone());
                }
                if request_token.is_none()
                    && !app.session.native_sso_manual.is_empty()
                    && auth::callback_token_matches(&app.session.native_sso_manual, &token_param)
                {
                    request_token = Some(app.session.native_sso_manual.clone());
                }
                let Some(rt) = request_token else {
                    app.save_session();
                    println!(
                        "login link stored, but its login token doesn't match the login started in this app — press Login here first, or use --har"
                    );
                    std::process::exit(1);
                };
                match auth::exchange(&rt, &app.session.native_sso_etoken.clone(), &blob) {
                    Ok(bearer) => {
                        app.session.token = bearer;
                        app.session.frl_request.clear();
                        app.session.frl_blob.clear();
                        app.save_session();
                        println!("login token saved");
                    }
                    Err(e) => {
                        app.session.callback_attempted = true;
                        app.save_session();
                        println!("callback stored, login token failed: {e}");
                        println!("retry with: iris-launcher --url \"{url}\" or the Apply button in settings");
                        std::process::exit(1);
                    }
                }
            }
            None => {
                eprintln!("the login link needs a login token and a login code");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if args.get(1).map(|a| a == "--har").unwrap_or(false) {
        // Chrome/Firefox drop protocol redirects silently and the vendor wipes the
        // console, so the callback only survives in a Preserve-log HAR export.
        // The approved endpoint returns the redirect inside its JSON response.
        let Some(path) = args.get(2) else {
            eprintln!("missing har file path");
            return Ok(());
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("cannot read {path}");
            return Ok(());
        };
        let Ok(har) = serde_json::from_str::<serde_json::Value>(&text) else {
            eprintln!("{path} is not valid HAR json");
            return Ok(());
        };
        let mut found: Option<String> = None;
        if let Some(entries) = har
            .get("log")
            .and_then(|l| l.get("entries"))
            .and_then(|e| e.as_array())
        {
            for e in entries {
                let url = e
                    .get("request")
                    .and_then(|r| r.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("");
                if !url.contains("native_sso/approved") {
                    continue;
                }
                let body = e
                    .get("response")
                    .and_then(|r| r.get("content"))
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                // Comet wraps responses as `for (;;);{...}` — strip the guard.
                let json = body.strip_prefix("for (;;);").unwrap_or(body);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                    if let Some(uri) = v
                        .get("payload")
                        .and_then(|p| p.get("uri"))
                        .and_then(|u| u.as_str())
                    {
                        if uri.contains("frl_login") {
                            found = Some(uri.to_string());
                        }
                    }
                }
            }
        }
        match found {
            Some(uri) => {
                let mut app = App::new(false);
                let Some((token_param, blob)) = auth::callback_params(&uri) else {
                    println!(
                        "the login link in that file has no login token or login code to apply"
                    );
                    return Ok(());
                };
                println!("callback found in HAR: login token and login code extracted");
                {
                    auth::log_line(&format!(
                        "callback via --har token={token_param} blob={blob}"
                    ));
                    app.session.frl_request = token_param.clone();
                    app.session.frl_blob = blob.clone();
                    let mut request_token: Option<String> = None;
                    if !app.session.native_sso_token.is_empty()
                        && auth::callback_token_matches(&app.session.native_sso_token, &token_param)
                    {
                        request_token = Some(app.session.native_sso_token.clone());
                    }
                    if request_token.is_none()
                        && !app.session.native_sso_manual.is_empty()
                        && auth::callback_token_matches(
                            &app.session.native_sso_manual,
                            &token_param,
                        )
                    {
                        request_token = Some(app.session.native_sso_manual.clone());
                    }
                    let Some(rt) = request_token else {
                        app.save_session();
                        println!("login link stored, but no login token matches its token param - press Login in this app first");
                        std::process::exit(1);
                    };
                    match auth::exchange(&rt, &app.session.native_sso_etoken.clone(), &blob) {
                        Ok(bearer) => {
                            app.session.token = bearer;
                            app.session.frl_request.clear();
                            app.session.frl_blob.clear();
                            app.save_session();
                            println!("login token saved");
                        }
                        Err(e) => {
                            app.session.callback_attempted = true;
                            app.save_session();
                            println!("login token failed: {e}");
                            std::process::exit(1);
                        }
                    }
                }
            }
            None => println!(
                "no frl_login callback in this HAR — check Preserve log was on and login completed"
            ),
        }
        return Ok(());
    }
    let settings_at_start = args.iter().any(|a| a == "--settings-page");
    // Low-VRAM or broken GL drivers segfault inside the graphics context
    // before any app code runs — the log stays empty because the window is
    // created before App::new. --safe forces software rendering; --no-covers
    // keeps the library grid light (plain grey cards instead of uploads).
    let safe_mode = args.iter().any(|a| a == "--safe")
        || std::env::var("IRIS_SAFE")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    #[cfg(target_os = "windows")]
    {
        let _ = safe_mode;
    }
    let no_covers = args.iter().any(|a| a == "--no-covers")
        || std::env::var("IRIS_NO_COVERS")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    // The log starts fresh each run (it was growing without bound when every
    // session appended to the old one). The path must exist before these lines
    // are written, App::new sets it only after the window is already created.
    std::fs::create_dir_all(data_dir()).ok();
    auth::init_log(
        data_dir()
            .join("launcher.log")
            .to_string_lossy()
            .into_owned(),
        true,
    );
    if safe_mode {
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        // On Wayland sessions winit picks the Wayland EGL backend, and NVIDIA
        // drivers crash there even for a plain window. Dropping WAYLAND_DISPLAY
        // makes the window go through X11 via XWayland instead.
        std::env::remove_var("WAYLAND_DISPLAY");
        auth::log_line("safe mode: software rendering forced, Wayland backend skipped");
    }
    // --x11 does the Wayland switch alone, without software rendering.
    if args.iter().any(|a| a == "--x11") {
        std::env::remove_var("WAYLAND_DISPLAY");
        auth::log_line("forcing X11 backend (--x11)");
    }
    if no_covers {
        auth::log_line("covers disabled (--no-covers)");
    }
    // Recorded BEFORE the window is built, so a graphics-driver segfault is
    // distinguishable from a crash inside the app.
    auth::log_line("gui starting: window + graphics context");
    let make_opts = || {
        let mut opts = eframe::NativeOptions::default();
        // Default 800x600 crops the cover grid on wide screens; give the library
        // room to breathe from the first frame.
        opts.viewport.inner_size = Some(egui::Vec2::new(1050.0, 720.0));
        // eframe's fallback window/taskbar icon is the egui logo (white "e" on
        // black) — ship the real logo instead. winit drops the alpha when drawing
        // the Windows taskbar icon, so transparent pixels would land as a white
        // box there: bake them onto the app's dark grey on Windows. Elsewhere the
        // alpha is kept and the logo blends on any background.
        if let Ok(img) = image::load_from_memory(include_bytes!("../../assets/iris-logo.png")) {
            let rgba = img.to_rgba8();
            let w = rgba.width();
            let h = rgba.height();
            #[cfg(target_os = "windows")]
            let raw = {
                let mut raw = rgba.into_raw();
                for i in (0..raw.len()).step_by(4) {
                    if raw[i + 3] == 0 {
                        raw[i] = 38;
                        raw[i + 1] = 38;
                        raw[i + 2] = 38;
                        raw[i + 3] = 255;
                    }
                }
                raw
            };
            #[cfg(not(target_os = "windows"))]
            let raw = rgba.into_raw();
            opts.viewport.icon = Some(std::sync::Arc::new(egui::IconData {
                rgba: raw,
                width: w,
                height: h,
            }));
        }
        opts
    };
    let res = {
        let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            eframe::run_native(
                "Iris Game Launcher",
                make_opts(),
                Box::new(move |_cc| {
                    // Log starts fresh per run (init_log truncated before the
                    // window), and the "gui starting" line written above proves
                    // where a graphics-context segfault happened.
                    let mut app = App::new(false);
                    app.settings_page = settings_at_start;
                    app.covers_off = no_covers;
                    Ok(Box::new(app))
                }),
            )
        }));
        match attempt {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(e)) => {
                auth::log_line(&format!("window attempt failed: {e:?}"));
                Err(e)
            }
            Err(_) => {
                auth::log_line("startup panicked while creating the window");
                Err(eframe::Error::AppCreation(Box::new(std::io::Error::other(
                    "the launcher panicked while creating the window",
                ))))
            }
        }
    };
    res
}

/// A superseded lookup must not overwrite the newer answer already in the
/// slot: poll_size takes the newest result once, and losing it would leave
/// the popup stuck "measuring…" with Install permanently disabled.
fn size_write_allowed(held: Option<usize>, gen: usize) -> bool {
    match held {
        Some(g) => gen >= g,
        None => true,
    }
}

#[cfg(test)]
/// Tests that mutate process-wide env vars (HOME/XDG_DATA_HOME) must not
/// race each other — the loaders read those vars on every App::new.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

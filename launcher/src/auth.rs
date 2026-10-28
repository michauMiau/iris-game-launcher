pub(crate) use sha2::{Digest, Sha256};
use std::io::Read;
use std::process::Command;

/// Windows spawns (reg, cmd) are console apps: without CREATE_NO_WINDOW they
/// open a visible console window behind the launcher.
#[cfg(target_os = "windows")]
fn quiet(cmd: Command) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = cmd;
    c.creation_flags(0x0800_0000);
    c
}

/// Native app id + public client token of the vendor Horizon Store PC client
/// ("skyline"). Both ship inside the vendor's own binaries and are public constants,
/// not user secrets. Verified live: webview_tokens_query accepts this token and
/// returns native SSO tokens (2026-10-23 probe).
pub const NATIVE_APP_ID: &str = "512466987071624";
const CLIENT_TOKEN: &str = "FRL|512466987071624|01d4a1f7fd0682aea7ee8ae987704d63";
/// doc_id of the xfr_create_profile_token mutation (FRLNuxCreateProfileTokenMutation
/// found verbatim in LibOVRPlatformImpl64_1.dll strings).
const PROFILE_TOKEN_DOC_ID: &str = "5787825127910775";
const PROFILE_TOKEN_APP_ID: &str = "1582076955407037";
/// Shared HTTP agent WITH timeouts: ureq's default has none, so a stalled
/// CDN connection hangs the worker forever — the slot never gets a result,
/// dl_inflight/fetch_inflight stick true, Cancel does nothing, and every
/// later install/fetch is refused until an app restart.
fn agent() -> &'static ureq::Agent {
    static A: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    A.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(10))
            .timeout_read(std::time::Duration::from_secs(30))
            .build()
    })
}

/// Append-only debug log in the launcher's data dir (launcher.log). The blob is
/// one-shot and expires server-side, so recording it locally costs nothing and
/// lets us replay/inspect failed exchanges without a console.
static LOG_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn init_log(path: String, truncate: bool) {
    if truncate {
        // Only the current run's log should remain; the GUI truncates at start.
        std::fs::write(&path, "").ok();
    }
    let _ = LOG_PATH.set(path);
}

pub fn log_path() -> Option<String> {
    LOG_PATH.get().cloned()
}

pub fn log_line(msg: &str) {
    let Some(path) = LOG_PATH.get() else { return };
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{now}] {msg}");
    }
}

/// Redact long secrets to a prefix so the log is useful but not a token dump.
pub fn log_peek(s: &str) -> String {
    if s.len() > 12 {
        format!("{}... ({len})", &s[..12], len = s.len())
    } else {
        s.to_string()
    }
}

fn post_json(url: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
    let payload = serde_json::to_string(&body).map_err(|e| format!("json encode: {e}"))?;
    // The vendor surfaces JSON error bodies on non-2xx statuses (e.g. 500 +
    // {"error":{...}} on blob decrypt failures), so read those bodies too.
    let resp = match agent()
        .post(url)
        .set("Content-Type", "application/json")
        .set("User-Agent", "Mozilla/5.0")
        .send(std::io::Cursor::new(payload.clone().into_bytes()))
    {
        Ok(r) => r,
        Err(ureq::Error::Status(_code, r)) => r,
        Err(e) => {
            log_line(&format!("POST {url} transport error: {e}"));
            return Err(format!("{url}: {e}"));
        }
    };
    let status = resp.status();
    let mut text = String::new();
    resp.into_reader()
        .read_to_string(&mut text)
        .map_err(|e| format!("read body from {url}: {e}"))?;
    // Minted profile tokens come back in the response body; never let them
    // (or the pasted login params) land in launcher.log.
    log_line(&format!("POST {url} -> {status} (body not logged)"));
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("bad json from {url}: {e}: {:.80}", text))?;
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("error");
        let code = err.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
        return Err(format!("{msg} (code {code})"));
    }
    Ok(v)
}

fn post_form(url: &str, body: &str) -> Result<serde_json::Value, String> {
    let resp = match agent()
        .post(url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("User-Agent", "Mozilla/5.0")
        .send_string(body)
    {
        Ok(r) => r,
        Err(ureq::Error::Status(_code, r)) => r,
        Err(e) => {
            log_line(&format!("POST {url} transport error: {e}"));
            return Err(format!("{url}: {e}"));
        }
    };
    let status = resp.status();
    let mut text = String::new();
    resp.into_reader()
        .read_to_string(&mut text)
        .map_err(|e| format!("read body from {url}: {e}"))?;
    // The request body carries the login token and login code, the response
    // can carry a minted token — never put either in launcher.log.
    log_line(&format!(
        "POST {url} -> {status} (request and response bodies not logged)"
    ));
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("bad json from {url}: {e}: {:.80}", text))?;
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("error");
        let code = err.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
        return Err(format!("{msg} (code {code})"));
    }
    Ok(v)
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Step 1 of the login: fetch native SSO tokens from the vendor, return the browser
/// URL the user logs into (browser handles password + 2FA) plus the
/// native_sso_token the client keeps for the redeem step. The vendor's own client
/// sends that token as `request_token` when decrypting the callback blob, and
/// the callback URL token param is sha256(native_sso_token) truncated to 16
/// hex chars (verified in app.asar parseDeeplinkURL + decryptBlob).
pub fn start_login() -> Result<(String, String, String), String> {
    let v = post_json(
        "https://meta.graph.meta.com/webview_tokens_query",
        serde_json::json!({ "access_token": CLIENT_TOKEN }),
    )?;
    let etoken = v
        .get("native_sso_etoken")
        .and_then(|x| x.as_str())
        .ok_or("response had no native_sso_etoken")?;
    let sso_token = v
        .get("native_sso_token")
        .and_then(|x| x.as_str())
        .ok_or("response had no native_sso_token")?;
    Ok((
        format!(
            "https://auth.meta.com/native_sso/confirm?native_app_id={}&native_sso_etoken={}&utm_source=skyline_splash",
            NATIVE_APP_ID,
            urlencode(etoken)
        ),
        sso_token.to_string(),
        etoken.to_string(),
    ))
}

/// True when the callback token param matches sha256(native_sso_token)[..16],
/// i.e. the callback really came from a login this app started.
pub fn callback_token_matches(native_sso_token: &str, token_param: &str) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(native_sso_token.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    let t16 = &hex[..16];
    if token_param.len() != 16 {
        return false;
    }
    t16 == token_param
}

/// Percent-decode over raw bytes, then UTF-8: decoding as chars would mangle
/// any non-ASCII value the callback carries.
fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Keep only characters that can legitimately appear in a param value:
/// percent-decoding plus trimming whitespace and quotes is not enough, a
/// protocol-handler argument can end with a stray apostrophe (observed in
/// launcher.log: blob ended with `'`, which made every decrypt return code 1).
fn clean_value(v: &str, allowed: &str) -> String {
    let dec = urldecode(v);
    dec.trim()
        .trim_matches(|c: char| c == '\'' || c == '"' || c == '`' || c.is_whitespace())
        .chars()
        .filter(|c| allowed.contains(*c))
        .collect()
}

/// Extract token= and blob= params from a callback URL like
/// oculus://link/frl_login?token=XXX&blob=YYY, percent-decoding both values
/// (auth.meta percent-encodes params in the redirect URL) and stripping junk
/// that protocol handlers or pastes can attach.
pub fn callback_params(url: &str) -> Option<(String, String)> {
    let q = url.split_once('?')?.1;
    let mut token = None;
    let mut blob = None;
    for part in q.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            if k == "token" {
                // The vendor can hand the hash in uppercase; the match gate
                // lowercases the sha256 hex it compares against, so lowercase
                // here instead of dropping the param.
                token = Some(clean_value(v, "0123456789abcdefABCDEF").to_lowercase());
            } else if k == "blob" {
                blob = Some(clean_value(
                    v,
                    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
                ));
            }
        }
    }
    let t = token.filter(|t| t.len() == 16)?;
    let b = blob.filter(|b| !b.is_empty())?;
    Some((t, b))
}

/// Steps 2+3: decrypt the callback blob into a first access token, then mint
/// the long-lived user (bearer) token via xfr_create_profile_token.
/// The vendor returns the same generic code 1 for every payload mismatch, so the
/// decrypt step tries the known payload shapes in turn and reports which ones
/// were attempted if all fail.
pub fn exchange(request_token: &str, etoken: &str, blob: &str) -> Result<String, String> {
    // Blob may not be server-side ready the instant the redirect fires, so
    // retry rounds with a short gap. The vendor returns the same generic code 1 for
    // every mismatch, hence the payload shapes are varied too.
    let mut attempts = Vec::new();
    let mut first: Option<String> = None;
    for round in 0..3u32 {
        if first.is_some() {
            break;
        }
        if round > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let mut shapes: Vec<(&str, serde_json::Value)> = vec![
            (
                "json body, native_sso_token",
                serde_json::json!({
                    "access_token": CLIENT_TOKEN,
                    "blob": blob,
                    "request_token": request_token,
                }),
            ),
            (
                "form body, native_sso_token",
                serde_json::json!({
                    "access_token": CLIENT_TOKEN,
                    "blob": blob,
                    "request_token": request_token,
                    "_form": true,
                }),
            ),
        ];
        if !etoken.is_empty() {
            shapes.push((
                "json body, native_sso_etoken",
                serde_json::json!({
                    "access_token": CLIENT_TOKEN,
                    "blob": blob,
                    "request_token": etoken,
                }),
            ));
        }
        for (label, body) in shapes {
            let is_form = body.get("_form").and_then(|x| x.as_bool()).unwrap_or(false);
            let result = if is_form {
                let form = format!(
                    "access_token={}&blob={}&request_token={}",
                    urlencode(CLIENT_TOKEN),
                    urlencode(blob),
                    urlencode(request_token)
                );
                post_form("https://meta.graph.meta.com/webview_blobs_decrypt", &form)
            } else {
                post_json("https://meta.graph.meta.com/webview_blobs_decrypt", body)
            };
            match result {
                Ok(v) => {
                    if let Some(t) = v.get("access_token").and_then(|x| x.as_str()) {
                        first = Some(t.to_string());
                        break;
                    }
                    attempts.push(format!("{label}: no access_token in response"));
                }
                Err(e) => attempts.push(format!("{label}: {e}")),
            }
        }
    }
    let first = match first {
        Some(t) => t,
        None => {
            log_line(&format!(
                "exchange: all shapes failed — {}",
                attempts.join(" | ")
            ));
            return Err(
                "the login code could not be applied — press Login once and apply the link it opens right away".to_string(),
            );
        }
    };
    log_line(&format!(
        "exchange: decrypt ok request={} blob={} etoken tried={}",
        log_peek(request_token),
        log_peek(blob),
        !etoken.is_empty()
    ));
    let v = post_json(
        "https://meta.graph.meta.com/graphql",
        serde_json::json!({
            "access_token": first,
            "doc_id": PROFILE_TOKEN_DOC_ID,
            "variables": format!("{{\"app_id\":\"{PROFILE_TOKEN_APP_ID}\"}}"),
        }),
    )?;
    let bearer = v
        .get("data")
        .and_then(|d| d.get("xfr_create_profile_token"))
        .and_then(|x| x.get("profile_tokens"))
        .and_then(|a| a.get(0))
        .and_then(|t| t.get("access_token"))
        .and_then(|x| x.as_str())
        .ok_or("profile token mint returned no access_token")?;
    Ok(bearer.to_string())
}

/// Whether the launcher currently owns the oculus:// callback.
/// "launcher" only when the desktop entry Exec points at THIS running binary,
/// an entry left over from another downloaded copy counts as "other".
#[cfg(target_os = "linux")]
pub fn handler_owner() -> &'static str {
    let dir = std::env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let entry = std::path::Path::new(&dir)
        .join("applications")
        .join("iris-handler.desktop");
    let Ok(text) = std::fs::read_to_string(&entry) else {
        return "none";
    };
    let exec = match text.lines().find(|l| l.starts_with("Exec=")) {
        // Strip quotes per token, not across the whole Exec line: trimming
        // quotes off the ends of `"exe" --url "%u"` left `exe"` as the first
        // token, which never matched the own path — Linux then claimed the
        // handler belonged to "another copy" forever.
        Some(line) => line["Exec=".len()..]
            .split_whitespace()
            .next()
            .map(|tok| tok.trim_matches('"').to_string())
            .unwrap_or_default(),
        None => return "other",
    };
    let mine = match std::env::var("APPIMAGE") {
        Ok(p) => p,
        Err(_) => std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    };
    if exec == mine {
        "launcher"
    } else {
        "other"
    }
}

/// Human-readable handler state: what oculus:// currently points at, if anything.
#[cfg(target_os = "linux")]
pub fn handler_status() -> String {
    let dir = std::env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let entry = std::path::Path::new(&dir)
        .join("applications")
        .join("iris-handler.desktop");
    match std::fs::read_to_string(&entry) {
        Ok(text) => {
            let exec = text
                .lines()
                .find(|l| l.starts_with("Exec="))
                .map(|l| l["Exec=".len()..].trim())
                .unwrap_or("?");
            format!("set, oculus:// -> {exec}")
        }
        Err(_) => "not set — the browser login link goes nowhere".into(),
    }
}

#[cfg(target_os = "linux")]
pub fn uninstall_oculus_handler() -> Result<String, String> {
    let dir = std::env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let apps_dir = std::path::Path::new(&dir).join("applications");
    let entry = apps_dir.join("iris-handler.desktop");
    if entry.exists() {
        std::fs::remove_file(&entry).map_err(|e| format!("remove desktop entry: {e}"))?;
    }
    let legacy = apps_dir.join("meta-pcvr-handler.desktop");
    if legacy.exists() {
        std::fs::remove_file(&legacy).ok();
    }
    let _ = Command::new("update-desktop-database")
        .arg(&apps_dir)
        .spawn();
    Ok("oculus:// handed back to the system default".into())
}

/// Register this launcher as the handler for oculus:// callback URLs.
/// For AppImage runs `APPIMAGE` holds the real file path; the temporary
/// /tmp/.mount_* dir dies when the AppImage exits, so never point at it.
#[cfg(target_os = "linux")]
pub fn install_oculus_handler() -> Result<String, String> {
    let exec = match std::env::var("APPIMAGE") {
        Ok(p) => std::path::PathBuf::from(p),
        Err(_) => std::env::current_exe().map_err(|e| format!("cannot resolve own exe: {e}"))?,
    };
    let dir = std::env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    let apps_dir = std::path::Path::new(&dir).join("applications");
    std::fs::create_dir_all(&apps_dir).map_err(|e| format!("mkdir applications: {e}"))?;
    let entry = apps_dir.join("iris-handler.desktop");
    // Drop the old brand-named entry so no stale handler grabs callbacks.
    let legacy = apps_dir.join("meta-pcvr-handler.desktop");
    if legacy.exists() {
        std::fs::remove_file(&legacy).ok();
    }
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=Iris Game Launcher login link\nExec=\"{}\" --url \"%u\"\nMimeType=x-scheme-handler/oculus\nTerminal=false\nNoDisplay=true\n",
        exec.display()
    );
    std::fs::write(&entry, content).map_err(|e| format!("write desktop entry: {e}"))?;
    let _ = Command::new("update-desktop-database")
        .arg(&apps_dir)
        .spawn();
    Ok(format!("{} -> {}", entry.display(), exec.display()))
}

/// Windows: register oculus:// under HKCU\Software\Classes, which the shell
/// resolves before the machine-level HKCR entry the vendor desktop app writes.
/// The vendor's own handler stays untouched under HKLM; deleting the HKCU key
/// gives control back to the vendor client process.
#[cfg(target_os = "windows")]
pub fn install_oculus_handler() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot resolve own exe: {e}"))?;
    let cmd = format!("\"{}\" --url \"%1\"", exe.display());
    quiet(Command::new("reg"))
        .args([
            "add",
            "HKCU\\Software\\Classes\\oculus",
            "/ve",
            "/d",
            "URL:Iris Launcher",
            "/f",
        ])
        .spawn()
        .map_err(|e| format!("reg add type: {e}"))?;
    let out = quiet(Command::new("reg"))
        .args([
            "add",
            "HKCU\\Software\\Classes\\oculus\\shell\\open\\command",
            "/ve",
            "/d",
            &cmd,
            "/f",
        ])
        .output()
        .map_err(|e| format!("reg add command: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "reg add failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(format!("HKCU\\Software\\Classes\\oculus -> {cmd}"))
}

#[cfg(target_os = "windows")]
pub fn handler_owner() -> &'static str {
    // Ownership must compare the command value, like the Linux desktop-entry
    // owner does: a leftover HKCU entry pointing at another exe is "other",
    // not "launcher", even though the key exists.
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let out = quiet(Command::new("reg"))
        .args([
            "query",
            "HKCU\\Software\\Classes\\oculus\\shell\\open\\command",
            "/ve",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            // Registry casing can differ from the exe path casing; compare
            // case-insensitively like handler_status does.
            let text = String::from_utf8_lossy(&o.stdout).to_lowercase();
            if text.contains(&exe.to_lowercase()) {
                "launcher"
            } else {
                "other"
            }
        }
        _ => "official",
    }
}

/// Human-readable handler state on Windows.
#[cfg(target_os = "windows")]
pub fn handler_status() -> String {
    let out = quiet(Command::new("reg"))
        .args([
            "query",
            "HKCU\\Software\\Classes\\oculus\\shell\\open\\command",
            "/ve",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            // The value line looks like: "…\iris-launcher.exe" --url "%1".
            // Take everything after REG_SZ — next_back() used to grab "%1",
            // which never matches the exe and made the launcher deny its own
            // handler.
            let cmd = text
                .lines()
                .find(|l| l.contains("REG_SZ"))
                .and_then(|l| l.split("REG_SZ").nth(1))
                .map(|v| v.trim().to_string())
                .unwrap_or_else(|| "?".to_string());
            let exe = std::env::current_exe()
                .map(|p| p.display().to_string().to_lowercase())
                .unwrap_or_default();
            let cmd_low = cmd.to_lowercase();
            // "set to this launcher" is only true when the key points at THIS
            // running exe; a leftover key pointing at another copy used to
            // claim ownership anyway.
            if cmd_low.contains(&exe) {
                format!("set to this launcher, oculus:// -> {cmd}")
            } else {
                format!("set, but to a different program: {cmd}")
            }
        }
        _ => "not set here, oculus:// goes to the official Link app, not this launcher".into(),
    }
}

#[cfg(target_os = "windows")]
pub fn uninstall_oculus_handler() -> Result<String, String> {
    let out = quiet(Command::new("reg"))
        .args(["delete", "HKCU\\Software\\Classes\\oculus", "/f"])
        .output()
        .map_err(|e| format!("reg delete: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "reg delete failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok("oculus:// handed back to the official Link app".into())
}

pub fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("xdg-open failed: {e}"))
    }
    #[cfg(target_os = "windows")]
    {
        quiet(Command::new("cmd"))
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("start failed: {e}"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err("no browser opener for this platform".into())
    }
}

/// Run PowerShell hidden (the Windows system dialogs need STA powershell).
#[cfg(target_os = "windows")]
pub fn quiet_powershell(script: &str) -> Result<std::process::Output, String> {
    let mut cmd = quiet(Command::new("powershell"));
    cmd.args(["-NoProfile", "-Sta", "-Command", script]);
    cmd.output().map_err(|e| format!("powershell: {e}"))
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
pub(crate) use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Horizon client UA the unofficial API libs send; store accepts it.
const UA: &str = "Dalvik/2.1.0 (Linux; U; Android 5.1.1; SM-G920L Build/LMY47X) [FBAN/OculusHorizon;FBAV/1.10.0;FBDV/SM-G920L;FBCR/SKTelecom;FBLC/ko_KR;FBSV/5.1.1;FBBD/samsung;FBBV/25632194;FBCA/armeabi-v7a:armeabi;FBMF/samsung;FBPN/com.oculus.horizon;]";

/// Public store app token of the vendor store. Store browse metadata works without a
/// user login; only entitlement needs the user bearer.
pub const STORE_TOKEN: &str = "OC|1076686279105243|";
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

fn post_json(url: &str, body: &str) -> Result<Value, String> {
    match agent()
        .post(url)
        .set("Content-Type", "application/json")
        .set("TE", "Trailers")
        .set("User-Agent", UA)
        .send_string(body)
    {
        Ok(resp) => {
            let mut text = String::new();
            resp.into_reader()
                .read_to_string(&mut text)
                .map_err(|e| format!("read: {e}"))?;
            serde_json::from_str(&text)
                .map_err(|e| format!("json: {e}: {}", text.chars().take(200).collect::<String>()))
        }
        Err(ureq::Error::Status(code, resp)) => {
            let mut text = String::new();
            resp.into_reader().read_to_string(&mut text).ok();
            // Never dump raw store JSON into a UI status: it can carry the
            // access token. Show the store's own message, or a plain refusal.
            let msg = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| {
                    v.get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| format!("the store refused the request (code {code})"));
            Err(msg)
        }
        Err(e) => Err(format!("request failed: {e}")),
    }
}

/// POST graph.oculus.com/graphql with a persisted doc_id. The old template
/// `q=` GET path is refused server-side now (HTTP 400), doc_id POSTs are the
/// live route. A rejected bearer is reported directly — the public store
/// token fallback was removed because an empty viewer read as "your games
/// are gone".
/// A store error is a network blip (not a token rejection) when it comes
/// from the transport layer: ureq timeout/DNS wording, a truncated read, or
/// a body that isn't JSON. Pure so it can be mutation-tested.
pub(crate) fn transport_blip(e: &str) -> bool {
    e.starts_with("request failed") || e.starts_with("read:") || e.starts_with("json:")
}

fn graphql_doc(token: &str, doc_id: &str, variables: Value) -> Result<Value, String> {
    let vars = serde_json::to_string(&variables).map_err(|e| e.to_string())?;
    let body = serde_json::to_string(&serde_json::json!({
        "access_token": token.trim(),
        "doc_id": doc_id,
        "variables": vars,
    }))
    .map_err(|e| e.to_string())?;
    let first = post_json("https://graph.oculus.com/graphql", &body);
    if token.trim() == STORE_TOKEN {
        return first;
    }
    match first {
        Ok(v) => Ok(v),
        Err(e) => {
            // Retrying with the PUBLIC store token returned an empty viewer,
            // which read as "your games are gone" instead of "your login was
            // refused". Viewer docs must report the bearer failure directly.
            auth_log(&format!("doc {doc_id} with bearer failed: {e}"));
            // A timeout/DNS/bad-JSON blip is not a token rejection — telling
            // the user to log in again for a network hiccup is a lie.
            if transport_blip(&e) {
                Err("the store could not be reached - check your connection".to_string())
            } else {
                Err("the store did not accept this login token - log in again".to_string())
            }
        }
    }
}

/// Debug logging mirror of auth.rs; fetcher keeps its own minimal hook so
/// store failures land in the same launcher.log.
fn auth_log(line: &str) {
    crate::auth::log_line(&format!("fetcher: {line}"));
}

/// One release channel (LIVE, beta, custom) with its latest binary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub binary_id: String,
    pub version: String,
    #[serde(default)]
    pub version_code: u64,
}

/// Release channels for an app via doc_id 3828663700542720 (documented on
/// Codeberg store-API wiki). Live route is a JSON doc_id POST; the public
/// store token is used when the bearer gets rejected.
pub fn release_channels(bearer: &str, app_id: &str) -> Result<Vec<Branch>, String> {
    let data = graphql_doc(
        bearer,
        "3828663700542720",
        serde_json::json!({"applicationID": app_id}),
    )?;
    // The store answers HTTP 200 with an "errors" array for throttling, so
    // saying "no versions" here is a lie — say what actually happened.
    if let Some(err) = data["errors"].as_array().and_then(|a| a.first()) {
        let msg = err["message"].as_str().unwrap_or("unknown");
        if msg.contains("Rate limit") {
            return Err(
                "the store is throttling requests right now - try again in a minute".into(),
            );
        }
        return Err(format!("the store refused the versions request ({msg})"));
    }
    let nodes = data["data"]["node"]["release_channels"]["nodes"]
        .as_array()
        .ok_or("the store listed no PC versions for this game")?;
    Ok(nodes
        .iter()
        .filter_map(|n| {
            // Channels can list Android headset builds; this launcher installs
            // PC builds only, so non-PC binaries are not offered.
            if n["latest_supported_binary"]["__typename"].as_str() != Some("PCBinary") {
                return None;
            }
            let name = n["channel_name"]
                .as_str()
                .or_else(|| n["name"].as_str())?
                .to_string();
            Some(Branch {
                name,
                binary_id: n["latest_supported_binary"]["id"].as_str()?.to_string(),
                version: n["latest_supported_binary"]["version"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                version_code: n["latest_supported_binary"]["version_code"]
                    .as_u64()
                    .unwrap_or(0),
            })
        })
        .collect())
}

pub struct AppInfo {
    pub id: String,
    pub display_name: String,
    /// Store blurb; empty when the store node carries no description text.
    pub description: String,
    pub entitled: bool,
    pub binary_id: String,
    pub version: String,
    pub version_code: u64,
    pub cover_url: String,
}

/// Pick a blurb out of store nodes. Field names differ between the store
/// format doc (pdp_metadata translations) and entitlement items, so try each
/// shape; an absent description stays empty rather than inventing text.
fn pick_description(node: &serde_json::Value) -> String {
    for key in ["description_short", "short_description", "description"] {
        if let Some(t) = node[key].as_str() {
            return t.to_string();
        }
    }
    if let Some(nodes) = node["pdp_metadata"]["translations"]["nodes"].as_array() {
        for t in nodes {
            for key in ["description_short", "short_description", "description"] {
                if let Some(s) = t[key].as_str() {
                    return s.to_string();
                }
            }
        }
    }
    String::new()
}

/// Store metadata for one app via doc_id POSTs (store format doc gives name
/// and cover, release channels doc gives the latest PC binary).
pub fn app_info(bearer: &str, id: &str) -> Result<AppInfo, String> {
    let fmt = graphql_doc(
        bearer,
        "8571881679548867",
        serde_json::json!({"applicationID": id}),
    )?;
    let node = &fmt["data"]["node"];
    let branches = release_channels(bearer, id).unwrap_or_default();
    let live = branches
        .iter()
        .find(|b| b.name == "LIVE")
        .or_else(|| branches.first());
    // Title art lives in the en_US revision translations as image_type
    // APP_IMG_* (measured live 2026-10-23); landscape suits the grid cells,
    // square is the fallback, and the top-level cover_square_image.uri is
    // the last resort.
    let mut cover_url = String::new();
    for rev in ["lastRevision", "firstRevision"] {
        let tr = node[rev]["nodes"][0]["pdp_metadata"]["translations"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut images: Vec<(String, String)> = Vec::new();
        for t in &tr {
            if t["locale"].as_str() != Some("en_US") {
                continue;
            }
            for img in t["images"]["nodes"].as_array().cloned().unwrap_or_default() {
                if let (Some(kind), Some(uri)) = (img["image_type"].as_str(), img["uri"].as_str()) {
                    images.push((kind.to_string(), uri.to_string()));
                }
            }
        }
        // Measured live: portrait cover suits the vertical grid cards.
        // SMALL_LANDSCAPE ~600 KB, COVER_LANDSCAPE ~4 MB, portrait ~2 MB.
        for want in [
            "APP_IMG_COVER_PORTRAIT",
            "APP_IMG_COVER_SQUARE",
            "APP_IMG_SMALL_LANDSCAPE",
            "APP_IMG_COVER_LANDSCAPE",
        ] {
            if let Some((_, uri)) = images.iter().find(|(k, _)| k == want) {
                cover_url = uri.clone();
                break;
            }
        }
        if !cover_url.is_empty() {
            break;
        }
    }
    if cover_url.is_empty() {
        cover_url = node["cover_square_image"]["uri"]
            .as_str()
            .unwrap_or("")
            .to_string();
    }
    Ok(AppInfo {
        id: id.to_string(),
        display_name: node["display_name"].as_str().unwrap_or("").to_string(),
        description: pick_description(node),
        entitled: false,
        binary_id: live.map(|b| b.binary_id.clone()).unwrap_or_default(),
        version: live.map(|b| b.version.clone()).unwrap_or_default(),
        version_code: live.map(|b| b.version_code).unwrap_or(0),
        cover_url,
    })
}

/// Owned apps from the active entitlements doc (doc_id 4850747515044496,
/// ComputerElite store GraphQL lib GetActiveEntitelments). Needs the user
/// bearer; response is keyed viewer → user id → active_entitlements nodes.
/// PCVR launcher keeps only entries that have a PC binary; headset-native
/// entitlements cannot be downloaded as PC builds.
pub fn owned_apps(bearer: &str, max_apps: usize) -> Result<Vec<AppInfo>, String> {
    if bearer.trim().is_empty() {
        return Err("no bearer token".into());
    }
    let data = graphql_doc(bearer, "4850747515044496", serde_json::json!({}))?;
    if data["error"].is_object() {
        return Err(format!(
            "login refused by the store: {}",
            data["error"]["message"].as_str().unwrap_or("unknown")
        ));
    }
    // Live shape measured with a real bearer 2026-10-23: data.viewer.user
    // directly (no id-keyed second level), and owned games report
    // active_state PERMANENT (plus ACTIVE for time-limited grants).
    let nodes = data["data"]["viewer"]["user"]["active_entitlements"]
        .as_array()
        .or_else(|| data["data"]["viewer"]["user"]["active_entitlements"]["nodes"].as_array())
        .ok_or(
            "the store did not send your games list - the login token may have stopped working",
        )?;
    let mut owned = Vec::new();
    for n in nodes {
        if owned.len() >= max_apps {
            break;
        }
        let state = n["active_state"].as_str().unwrap_or("");
        if state != "PERMANENT" && state != "ACTIVE" {
            continue;
        }
        let item = &n["item"];
        let id = item["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let name = item["display_name"].as_str().unwrap_or("").to_string();
        let binary = &item["latest_supported_binary"];
        let pc_binary = binary["__typename"].as_str() == Some("PCBinary");
        let (binary_id, version, version_code) = if pc_binary {
            (
                binary["id"].as_str().unwrap_or("").to_string(),
                binary["version"].as_str().unwrap_or("").to_string(),
                binary["version_code"].as_u64().unwrap_or(0),
            )
        } else {
            // Headset-native / GearVR entitlements have no PC build at all.
            // Asking the channels doc for each of them burned the store's
            // rate limit and made real PC games report "no versions".
            continue;
        };
        if binary_id.is_empty() {
            continue;
        }
        let cover = item["cover_square_image"]["uri"]
            .as_str()
            .unwrap_or("")
            .to_string();
        owned.push(AppInfo {
            id,
            display_name: name,
            description: pick_description(item),
            entitled: true,
            binary_id,
            version,
            version_code,
            cover_url: cover,
        });
    }
    Ok(owned)
}

/// Error text without the URL: store URLs carry the login token.
fn store_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, _) => format!("the store answered with code {code}"),
        ureq::Error::Transport(_) => "the connection to the store failed".to_string(),
    }
}

/// Byte-counted fetch with cancel, for big tool downloads (Proton GE is
/// ~500 MB) — the game-style progress row needs byte counts and an abort.
/// Download a release asset, verifying the byte count against the server's
/// content-length. A GitHub release asset can end the stream early when the
/// network hiccups; ureq reports that as a clean EOF, so the old loop handed
/// a truncated archive to the zip parser and the user saw "Could not find
/// central directory end" instead of an honest download error. A short body
/// is retried, resumed with a Range request when the server answers 206.
pub fn fetch_bytes_watch(
    url: &str,
    progress: &std::sync::Arc<std::sync::Mutex<(usize, usize, u64, u64)>>,
    stop: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use std::sync::atomic::Ordering::Relaxed;
    let mut buf: Vec<u8> = Vec::new();
    let mut total = 0u64;
    for attempt in 0..3 {
        if stop.load(Relaxed) {
            return Err("download cancelled".into());
        }
        let mut req = agent().get(url).set("User-Agent", UA);
        let resuming = !buf.is_empty();
        if resuming {
            req = req.set("Range", &format!("bytes={}-", buf.len()));
        }
        let resp = match req.call() {
            Ok(r) => r,
            // A transport blip on a retry is not the download's verdict: the
            // loop re-attempts and the final error after all tries stays
            // honest. Once bytes are already on disk, a failed resume means
            // the stream died — report the truncation, not a connection
            // error (Windows runners reset resumed loopback connects and hid
            // the truncation verdict behind store_err wording).
            Err(e) => {
                if !buf.is_empty() {
                    break;
                }
                if attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_millis(600));
                    continue;
                }
                return Err(store_err(e));
            }
        };
        if resuming && resp.status() != 206 {
            // Server ignored the Range header: the full body follows, so the
            // partial bytes must not be glued onto it.
            buf.clear();
        }
        let part = resp
            .header("content-length")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        if buf.is_empty() {
            total = part;
        } else {
            total = buf.len() as u64 + part;
        }
        if buf.is_empty() {
            buf.reserve(total as usize);
        }
        let mut reader = resp.into_reader();
        let mut chunk = [0u8; 64 * 1024];
        loop {
            if stop.load(Relaxed) {
                return Err("download cancelled".into());
            }
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    let mut p = progress.lock().unwrap();
                    p.2 = buf.len() as u64;
                    p.3 = total;
                }
                // ureq raises "response body closed before all bytes were
                // read" when the stream dies mid-content-length. That is the
                // early-end case, retryable — not a hard failure.
                Err(_) => break,
            }
        }
        if total == 0 || buf.len() as u64 == total {
            return Ok(buf);
        }
        if attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(600));
        }
    }
    Err(format!(
        "download ended early: {} of {} bytes received after 3 tries",
        buf.len(),
        total
    ))
}

fn fetch_bytes(url: &str, bearer: &str) -> Result<Vec<u8>, String> {
    let bearer = bearer.trim();
    let mut req = agent().get(url).set("User-Agent", UA);
    if !bearer.is_empty() {
        let auth = if bearer.starts_with("Bearer ") {
            bearer.to_string()
        } else {
            format!("Bearer {bearer}")
        };
        req = req.set("Authorization", &auth);
    }
    // ureq's error Display includes the full URL, and these URLs embed the
    // login token — never let the URL into user-facing text.
    let resp = req.call().map_err(store_err)?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("cdn read: {e}"))?;
    Ok(buf)
}

pub fn download_cover(url: &str, dest: &Path) -> Result<(), String> {
    let url = if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_string()
    };
    let resp = agent()
        .get(&url)
        .set("User-Agent", "Mozilla/5.0")
        .call()
        .map_err(|e| format!("cover request: {e}"))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("cover read: {e}"))?;
    std::fs::write(dest, buf).map_err(|e| format!("cover write: {e}"))
}

/// Download manifest zip for a binary, extract manifest.json, then download
fn fetch_manifest(bearer: &str, binary_id: &str) -> Result<Value, String> {
    let manifest_url = format!(
        "https://securecdn.oculus.com/binaries/download/?id={binary_id}&access_token={}&get_manifest=1",
        bearer.trim()
    );
    let zip_bytes = fetch_bytes(&manifest_url, bearer)?;
    let mut reader = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| {
        crate::auth::log_line(&format!("fetcher: manifest zip unreadable: {e}"));
        "the store's file list was unreadable".to_string()
    })?;
    let names: Vec<String> = reader.file_names().map(|s| s.to_string()).collect();
    for name in names {
        if name.ends_with(".json") {
            let mut f = reader
                .by_name(&name)
                .map_err(|e| format!("zip entry: {e}"))?;
            let mut s = String::new();
            f.read_to_string(&mut s)
                .map_err(|e| format!("manifest read: {e}"))?;
            return serde_json::from_str(&s).map_err(|e| {
                crate::auth::log_line(&format!("fetcher: manifest json unreadable: {e}"));
                "the store's file list was unreadable".to_string()
            });
        }
    }
    Err("the store's file list had no contents".into())
}

/// How much space the bundle needs on disk: (file count, decompressed bytes).
/// Used by the install popup before anything is downloaded.
pub fn bundle_size(bearer: &str, binary_id: &str) -> Result<(usize, u64), String> {
    let manifest = fetch_manifest(bearer, binary_id)?;
    let files = manifest["files"]
        .as_object()
        .ok_or("manifest has no files map")?;
    let bytes: u64 = files
        .values()
        .map(|e| e["size"].as_u64().unwrap_or(0))
        .sum();
    Ok((files.len(), bytes))
}

/// True login check: the account id behind the bearer, fetched live.
pub fn account_id(bearer: &str) -> Result<u64, String> {
    let data = graphql_doc(bearer, "4850747515044496", serde_json::json!({}))?;
    if data["error"].is_object() {
        return Err(format!(
            "account probe rejected: {}",
            data["error"]["message"].as_str().unwrap_or("unknown")
        ));
    }
    let id = data["data"]["viewer"]["user"]["id"]
        .as_str()
        .ok_or("the store did not recognise this login token")?;
    id.parse::<u64>().map_err(|e| format!("bad viewer id: {e}"))
}

/// GitHub release list for the Proton GE / MelonLoader installers (public API,
/// no auth; unauthenticated rate limit is fine for a handful of calls).
/// `avoid` lists name fragments for other platforms' builds: store games are
/// Windows games (even when launched through Proton/Wine), so MelonLoader must
/// ship its Windows build; Proton GE itself must skip win/mac archives.
pub fn github_releases(
    repo: &str,
    avoid: &[&str],
) -> Result<Vec<(String, String, String)>, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=15");
    let req = agent()
        .get(&url)
        .set("User-Agent", "iris-launcher/0.1")
        .call();
    // Never surface ureq's raw debug text ("HTTP error status code: 403, …").
    let resp = req.map_err(|e| match e {
        ureq::Error::Status(code, _) => format!("GitHub refused the version list (code {code})"),
        ureq::Error::Transport(_) => "GitHub could not be reached".to_string(),
    })?;
    let mut s = String::new();
    resp.into_reader()
        .read_to_string(&mut s)
        .map_err(|e| format!("github read: {e}"))?;
    let body: Value = serde_json::from_str(&s).map_err(|e| format!("github json: {e}"))?;
    let out = body
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?.to_string();
            // Proton ships per-arch tar.gz; MelonLoader ships per-OS zips
            // (Linux.x64, x64, mac). Grabbing the first archive installed the
            // Linux build into Windows games, so choose by the running OS.
            let assets = r["assets"].as_array()?;
            let asset = pick_asset(assets, &[], avoid)?;
            Some((
                tag,
                asset["name"].as_str()?.to_string(),
                asset["browser_download_url"].as_str()?.to_string(),
            ))
        })
        .collect();
    Ok(out)
}

/// Choose the release asset for the running OS: prefer names mentioning this
/// OS, then any archive that does NOT mention another OS, then any archive.
/// Pure so it can be mutation-tested.
pub(crate) fn pick_asset<'a>(
    assets: &'a [Value],
    want: &[&str],
    avoid: &[&str],
) -> Option<&'a Value> {
    let name = |a: &Value| a["name"].as_str().unwrap_or("").to_lowercase();
    // .exe is NOT an archive here: MelonLoader v0.5.4 lists MelonLoader.Installer.exe
    // first, and treating it as an archive made the picker hand an NSIS binary to
    // the zip unpacker ("archive is incomplete (737280 bytes, no archive end
    // found)" — the exact Installer.exe asset size, fetched twice and via curl too).
    let is_arch = |n: &str| n.ends_with(".tar.gz") || n.ends_with(".zip");
    assets
        .iter()
        .find(|a| {
            let n = name(a);
            is_arch(&n)
                && want.iter().any(|w| n.contains(w))
                && !avoid.iter().any(|x| n.contains(x))
        })
        .or_else(|| {
            assets.iter().find(|a| {
                let n = name(a);
                is_arch(&n) && !avoid.iter().any(|x| n.contains(x))
            })
        })
        .or_else(|| assets.iter().find(|a| is_arch(&name(a))))
}

/// Download one GitHub asset then unpack it into dest_dir. tar.gz goes through
/// the system tar (Linux only); .zip goes through the zip crate.
/// Save a release asset verbatim to a file (NSIS installers are not archives).
#[cfg(target_os = "windows")]
pub fn save_release_file(
    url: &str,
    dest_path: &Path,
    progress: &std::sync::Arc<std::sync::Mutex<(usize, usize, u64, u64)>>,
    stop: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let bytes = fetch_bytes_watch(url, progress, stop)?;
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    std::fs::write(dest_path, bytes).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

pub fn install_release(
    url: &str,
    dest_dir: &Path,
    progress: &std::sync::Arc<std::sync::Mutex<(usize, usize, u64, u64)>>,
    stop: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    std::fs::create_dir_all(dest_dir).map_err(|e| format!("mkdir: {e}"))?;
    // A stream without a Content-Length can end clean at a partial body, so
    // the watch accepts it and the archive guard is what catches the cut.
    // Retrying the fetch is the honest recovery for that; a corrupt-but-complete
    // archive fails fast without retry.
    let mut last = String::new();
    for attempt in 0..3 {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("download cancelled".into());
        }
        let bytes = fetch_bytes_watch(url, progress, stop)?;
        let unpack = if url.ends_with(".tar.gz") {
            #[cfg(not(target_os = "windows"))]
            {
                unpack_tar(&bytes, dest_dir)
            }
            #[cfg(target_os = "windows")]
            Err("tar.gz needs a Linux host".to_string())
        } else {
            unpack_zip(bytes, dest_dir)
        };
        match unpack {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e;
                if !last.contains("cut off") && !last.contains("incomplete") {
                    return Err(last);
                }
                if attempt < 2 {
                    crate::auth::log_line(&format!(
                        "asset unpack hit a truncated archive ({last}); re-downloading (try {})",
                        attempt + 2
                    ));
                    std::thread::sleep(std::time::Duration::from_millis(800));
                }
            }
        }
    }
    // A deterministic cut at the same byte count every try is not a transient:
    // on user machines the in-process ureq stream was cut at exactly 720 KiB
    // while plain system curl delivered the full 19.2 MB. Redirect-chain and
    // chunked-body handling differ, so the system curl is the fallback before
    // declaring the asset unreachable.
    let tmp = std::env::temp_dir().join(format!("pcvr-curl-asset-{}", std::process::id()));
    let curl_ok = std::process::Command::new("curl")
        .args(["-sL", "--fail", "-o"])
        .arg(&tmp)
        .arg(url)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !curl_ok {
        std::fs::remove_file(&tmp).ok();
        return Err(format!(
            "{last} — retried 3 times, still cut off; system curl could not reach the asset either"
        ));
    }
    let bytes = std::fs::read(&tmp).map_err(|e| format!("read curl download: {e}"))?;
    std::fs::remove_file(&tmp).ok();
    crate::auth::log_line(&format!(
        "asset re-fetched via system curl ({} bytes) after the in-process stream was cut off",
        bytes.len()
    ));
    let mut p = progress.lock().unwrap();
    p.2 = bytes.len() as u64;
    p.3 = bytes.len() as u64;
    let unpacked = if url.ends_with(".tar.gz") {
        #[cfg(not(target_os = "windows"))]
        {
            unpack_tar(&bytes, dest_dir)
        }
        #[cfg(target_os = "windows")]
        Err("tar.gz needs a Linux host".to_string())
    } else {
        unpack_zip(bytes, dest_dir)
    };
    match unpacked {
        Ok(()) => Ok(()),
        Err(e) => Err(format!(
            "{last} — retried 3 times and system curl as well, still cut off: {e}"
        )),
    }
}

/// Unpack a tar.gz asset with the system tar (Linux), rejecting a corrupt
/// archive instead of silently leaving a half-unpacked tool dir.
#[cfg(not(target_os = "windows"))]
pub fn unpack_tar(bytes: &[u8], dest_dir: &Path) -> Result<(), String> {
    let tmp = std::env::temp_dir().join("pcvr-tar-asset.tar.gz");
    std::fs::write(&tmp, bytes).map_err(|e| format!("write asset: {e}"))?;
    let ok = std::process::Command::new("tar")
        .arg("-xf")
        .arg(&tmp)
        .arg("-C")
        .arg(dest_dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    std::fs::remove_file(&tmp).ok();
    if !ok {
        return Err("tar extract failed".into());
    }
    Ok(())
}

/// Unpack a GitHub release zip asset into dest_dir, keeping nested paths.
pub fn unpack_zip(bytes: Vec<u8>, dest_dir: &Path) -> Result<(), String> {
    if !bytes.windows(4).any(|w| w == b"PK\x05\x06") {
        // EOCD magic absent means the archive never reached its end.
        return Err(format!(
            "asset zip: archive is incomplete ({} bytes, no archive end found) — download was cut off",
            bytes.len()
        ));
    }
    let mut reader =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("asset zip: {e}"))?;
    for i in 0..reader.len() {
        let mut f = reader.by_index(i).map_err(|e| format!("zip entry: {e}"))?;
        let name = f.mangled_name();
        if name.as_os_str().is_empty() {
            continue;
        }
        let out = dest_dir.join(&name);
        if f.is_dir() {
            std::fs::create_dir_all(out).ok();
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).ok();
        }
        let mut w = std::fs::File::create(&out).map_err(|e| format!("create: {e}"))?;
        std::io::copy(&mut f, &mut w).map_err(|e| format!("unpack: {e}"))?;
    }
    Ok(())
}

/// Fetch a manifest zip and deflate every file segment into dest_dir.
/// Manifest format is the one ComputerElite's store GraphQL lib parses:
/// files map with size + segments. `progress(files_done, files_total,
/// bytes_done, bytes_total)` runs after each file so downloads report real
/// progress instead of downloading blind.
/// Verify and decompress one CDN segment. The wire stream starts with a
/// 2-byte header (Boneworks: proprietary header; Bonelab measured
/// 2026-10-24: zlib magic 78 9c), and the manifest per-segment sha256 rule
/// varies by manifest version: Boneworks hashed the DEFLATED payload after
/// the header, Bonelab hashes the DECOMPRESSED chunk. Accept either rule
/// (measured, not guessed), then write the decompressed chunk. A response
/// shorter than the header must reject cleanly: slicing it panicked the
/// download worker, leaving dl_inflight stuck true forever (frozen progress
/// bar, useless Cancel, later installs refused, app restart only recovery).
pub(crate) fn decode_segment(blob: &[u8], sha: &str, key: &str) -> Result<Vec<u8>, String> {
    if blob.len() < 2 {
        return Err(format!(
            "segment of {key} shorter than 2-byte header ({})",
            blob.len()
        ));
    }
    let payload = &blob[2..];
    let mut chunk = Vec::new();
    flate2::read::DeflateDecoder::new(payload)
        .read_to_end(&mut chunk)
        .map_err(|e| {
            crate::auth::log_line(&format!("fetcher: deflate failed for {key}: {e}"));
            format!("the store sent a broken file chunk for {key}")
        })?;

    let mut h_payload = Sha256::new();
    h_payload.update(payload);
    let mut h_chunk = Sha256::new();
    h_chunk.update(&chunk);
    let payload_sha = format!("{:x}", h_payload.finalize());
    let chunk_sha = format!("{:x}", h_chunk.finalize());
    if payload_sha != sha && chunk_sha != sha {
        return Err(format!("checksum mismatch for {key} — download again"));
    }
    Ok(chunk)
}

pub fn download_binary(
    bearer: &str,
    binary_id: &str,
    dest_dir: &Path,
    stop: &std::sync::atomic::AtomicBool,
    progress: &dyn Fn(usize, usize, u64, u64),
) -> Result<(usize, usize), String> {
    let manifest = fetch_manifest(bearer, binary_id)?;
    let files = manifest["files"]
        .as_object()
        .ok_or("manifest has no files map")?;
    let total_bytes: u64 = files
        .values()
        .map(|e| e["size"].as_u64().unwrap_or(0))
        .sum();

    std::fs::create_dir_all(dest_dir).map_err(|e| format!("mkdir: {e}"))?;
    // Hashes saved by a previous full download: files already on disk with a
    // matching hash+size are reused, so pressing Install on an installed game
    // (or re-running after a crash) never re-downloads intact content.
    let stored: serde_json::Map<String, Value> =
        match std::fs::read(dest_dir.join(".oculus-hashes.json")) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(v) => v.as_object().cloned().unwrap_or_default(),
                Err(_) => serde_json::Map::new(),
            },
            Err(_) => serde_json::Map::new(),
        };
    let mut files_done = 0usize;
    let mut bytes_done = 0u64;
    let mut reused = 0usize;
    let mut hashes: serde_json::Map<String, Value> = serde_json::Map::new();
    // Hash cache is written via tmp+rename: a plain overwrite can be read
    // torn by the Verify thread and then report "no checksums saved". A
    // serialization failure must not wipe the cache with empty bytes.
    fn save_hashes(dest_dir: &Path, hashes: &serde_json::Map<String, Value>) {
        let Ok(bytes) = serde_json::to_vec(&Value::Object(hashes.clone())) else {
            return;
        };
        let tmp = dest_dir.join(".oculus-hashes.json.tmp");
        if std::fs::write(&tmp, bytes).is_err() {
            return;
        }
        let _ = std::fs::rename(&tmp, dest_dir.join(".oculus-hashes.json"));
    }
    let seg_base = "https://securecdn.oculus.com/binaries/segment/";
    for (key, entry) in files {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("download cancelled".to_string());
        }
        // The store manifest paths use WINDOWS separators (`Data\Plugins\...`).
        // Replacing only '/' left the whole key as ONE flat filename on Linux
        // — files landed named `BONELAB_Oculus_Windows64_Data\app.info` with no
        // folders, which Unity cannot load. Split on BOTH separators and
        // rebuild with this platform's separator; refuse keys that escape the
        // install folder via `..`.
        let rel = normalize_manifest_key(key)?;
        let file_path = dest_dir.join(&rel);
        let size = entry["size"].as_u64().unwrap_or(0);
        let stored_sha = stored
            .get(&rel)
            .and_then(|v| v.get("sha256").and_then(|s| s.as_str()));
        if file_path.exists() {
            let size_ok = std::fs::metadata(&file_path)
                .map(|m| m.len() == size)
                .unwrap_or(false);
            let actual = file_sha256(&file_path).ok();
            if reusable(stored_sha, actual.as_deref(), size_ok) {
                hashes.insert(
                    rel,
                    serde_json::json!({ "sha256": stored_sha.unwrap_or(""), "size": size }),
                );
                files_done += 1;
                bytes_done += size;
                reused += 1;
                progress(files_done, files.len(), bytes_done, total_bytes);
                if files_done.is_multiple_of(25) {
                    // Cache survives a kill/restart: keep writing hashes as we
                    // go, so a crashed download resumes instead of starting over.
                    save_hashes(dest_dir, &hashes);
                }
                continue;
            }
        }
        if let Some(p) = file_path.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("create dir: {e}"))?;
        }
        let mut out =
            std::fs::File::create(&file_path).map_err(|e| format!("create {rel}: {e}"))?;
        let segments = entry["segments"]
            .as_array()
            .ok_or("the store's file list is missing pieces of a file")?;
        let mut file_hasher = Sha256::new();
        for seg in segments {
            // Cancel must also fire between SEGMENTS: a single multi-GB file
            // has many segments, and checking only between files means Cancel
            // waits for the whole file (and can never fire if a segment hung).
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("download cancelled".to_string());
            }
            let sha = seg[1]
                .as_str()
                .ok_or("the store did not send the checksum for a file chunk")?;
            let url = format!(
                "{seg_base}?access_token={}&binary_id={binary_id}&segment_sha256={sha}",
                bearer.trim()
            );
            let blob = fetch_bytes(&url, bearer)?;
            let chunk = decode_segment(&blob, sha, key)?;
            file_hasher.update(&chunk);
            std::io::Write::write_all(&mut out, &chunk).map_err(|e| format!("write {key}: {e}"))?;
        }
        hashes.insert(
            rel,
            serde_json::json!({
                "sha256": format!("{:x}", file_hasher.finalize()),
                "size": entry["size"].as_u64().unwrap_or(0),
            }),
        );
        files_done += 1;
        bytes_done += entry["size"].as_u64().unwrap_or(0);
        // Update the bar after every file: bundles with few large files would
        // otherwise sit at 0% until the last frame.
        progress(files_done, files.len(), bytes_done, total_bytes);
        if files_done.is_multiple_of(25) {
            // Same resumability rule for downloaded files: the hash cache must
            // survive a crash mid-bundle.
            save_hashes(dest_dir, &hashes);
        }
    }
    // Keep local content hashes for offline integrity checks later. The
    // manifest copy was never read anywhere, and a write failure there used to
    // report "download failed" after every file was already on disk.
    save_hashes(dest_dir, &hashes);
    Ok((files_done, reused))
}

/// Offline integrity check against the hashes saved at download time.
/// Returns (files_ok, files_bad, files_missing).
pub fn verify_files(dest_dir: &Path) -> Result<(usize, usize, usize), String> {
    let hashes_path = dest_dir.join(".oculus-hashes.json");
    let raw = std::fs::read(&hashes_path).map_err(|e| format!("read hashes: {e}"))?;
    let hashes: Value = serde_json::from_slice(&raw).map_err(|e| format!("hashes json: {e}"))?;
    let map = hashes.as_object().ok_or("hashes file has no object")?;
    let (mut ok, mut bad, mut missing) = (0usize, 0usize, 0usize);
    for (rel, entry) in map {
        let path = dest_dir.join(rel);
        // Streamed hash: Verify on a multi-GB install must not load every
        // asset into RAM at once.
        let Ok(actual) = file_sha256(&path) else {
            missing += 1;
            continue;
        };
        if actual == entry["sha256"].as_str().unwrap_or("") {
            ok += 1;
        } else {
            bad += 1;
        }
    }
    Ok((ok, bad, missing))
}

/// SHA-256 of a file on disk, streamed so multi-GB assets don't get read
/// into memory whole.
pub(crate) fn file_sha256(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// A file on disk may be reused (not re-downloaded) only when the stored
/// hash matches the actual hash AND the size matches the manifest entry.
/// Anything partial or corrupt falls through to a fresh download.
pub(crate) fn reusable(stored_sha: Option<&str>, actual_sha: Option<&str>, size_ok: bool) -> bool {
    match (stored_sha, actual_sha) {
        (Some(s), Some(a)) => size_ok && s == a,
        _ => false,
    }
}

/// Store manifest keys are Windows-style paths (`Data\Plugins\file.dll`).
/// Split on BOTH `/` and `\`, drop empty/`.` segments, rebuild with the
/// current platform's separator, and reject `..` (path traversal out of the
/// install folder).
pub(crate) fn normalize_manifest_key(key: &str) -> Result<String, String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in key.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(format!("manifest path escapes the install folder: {key}"));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(format!("manifest path is empty: {key}"));
    }
    Ok(parts.join(std::path::MAIN_SEPARATOR_STR))
}

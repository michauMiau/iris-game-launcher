use std::ffi::{c_char, c_int, c_void};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Build label emitted as the first trace line so logs self-identify which
/// shim build produced them (assets/ovrplugin-shim.dll is rebuilt from this
/// repo's sources; the label here is the source of truth, then git tag).
pub const SHIM_BUILD_LABEL: &str = "ovrplugin-shim 0.1.34-abi";

/// Lazy bring-up: Unity's Oculus integration polls GetInitialized without
/// ever calling Initialize on this plugin build (measured on Boneworks:
/// version reads then repeated GetInitialized polls, no Initialize record).
/// On the first poll, try loading LibOVRRT64_1.dll beside the exe and
/// creating a session; if it succeeds, report initialized true. Attempts
/// are recorded once, so the trace shows why a poll stayed false.
///
/// Probe mode: IRIS_OVR_FAKE_INIT=1 reports initialized immediately and
/// skips the real runtime attempt, so Unity proceeds to its follow-up
/// plugin calls without Revive/ovr_Initialize running in the background.
pub fn initialized() -> bool {
    if INITIALIZED.load(Ordering::Relaxed) {
        return true;
    }
    if fake_init_enabled() {
        return true;
    }
    lazy_runtime_once();
    INITIALIZED.load(Ordering::Relaxed)
}

pub fn set_initialized(v: bool) {
    INITIALIZED.store(v, Ordering::Relaxed);
}

/// Trace directory resolution, in order:
///   1. IRIS_OVR_TRACE_DIR (checked out-of-band by the user/launcher)
///   2. OVR_SHIM_LOG when it names a directory (shared preset slot) — when
///      it is a file path we keep appending into it, since the launcher
///      knows where it collected it.
///   3. process cwd (the game dir when launched by the launcher)
fn trace_dir() -> Option<String> {
    for key in ["IRIS_OVR_TRACE_DIR", "OVR_SHIM_LOG"] {
        if let Ok(v) = std::env::var(key) {
            if std::path::Path::new(&v).is_dir() {
                let s = v.replace('/', "\\").trim_end_matches(['\\', '/']).to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

fn log_path() -> String {
    let file = "iris-ovrplugin-trace.log".to_string();
    if let Some(dir) = trace_dir() {
        return format!("{dir}\\{file}");
    }
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::new());
    if cwd.is_empty() {
        file
    } else {
        format!("{cwd}\\{file}")
    }
}

/// Milliseconds since the DLL loaded, so traces compare against Unity log
/// timestamps across Windows and Proton runs.
fn now_ms() -> u64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

/// Mirrors the platform shim's record(): the trace goes wherever the
/// `OVR_SHIM_LOG` env var points (the launcher's "Shim trace log" switch
/// sets it per launch). Opening must never abort the game: a panic across
/// an FFI boundary is UB, so an unwritable path stays silently off.
fn record(name: &str) {
    // Gated by OVR_SHIM_LOG. The other shim's record() caches the file
    // handle in a OnceLock: if its first open failed, it stayed silent
    // for the whole session. Here every call re-opens, but a failed open
    // just means this write is skipped (a panic across the FFI boundary
    // is UB; the game keeps running either way). The very first record
    // TRUNCATES the file first, so the log the user collects is always
    // from THIS run (never leftover rows from an earlier one), then names
    // the shim build so logs self-identify the place and version.
    static BUILD_LABELED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let path = log_path();
    let first_run = !BUILD_LABELED.swap(true, std::sync::atomic::Ordering::Relaxed);
    let mut f = if first_run {
        OpenOptions::new().create(true).write(true).truncate(true).open(&path)
    } else {
        OpenOptions::new().create(true).append(true).open(&path)
    };
    // one more chance, append-mode, if the truncate open raced/lost
    let mut f = match f {
        Ok(f) => f,
        Err(_) => match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => f,
            Err(_) => return,
        },
    };
    if first_run {
        let _ = writeln!(f, "{} shim-build {}", now_ms(), SHIM_BUILD_LABEL);
    }
    let _ = writeln!(f, "{} {}", now_ms(), name);
    let _ = f.flush();
}

#[no_mangle]
pub extern "system" fn DllMain(_h: *mut std::os::raw::c_void, reason: u32, _r: *mut std::os::raw::c_void) -> i32 {
    if reason == 1 {
        record("ovrplugin-shim loaded");
    }
    1
}

pub fn now_sec() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

static FRAMES: AtomicU32 = AtomicU32::new(0);

pub fn trace_call(name: &str) {
    record(name);
}

pub fn begin_frame() -> i32 {
    // Frame traffic is high-volume: log the first three frames and then one
    // every 600 (~10 s at 60 Hz), like the platform shim's null-poll
    // heartbeat — liveness proof without log spam.
    let n = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if n <= 3 || n % 600 == 0 {
        record(&format!("begin_frame {n}"));
    }
    1 // ovrpBool.True: frame should render
}

pub fn end_frame() -> i32 {
    0 // ovrpResult.Success
}

type OvResult = c_int;
type FnOvrInitialize = unsafe extern "C" fn(*const c_void) -> OvResult;
type FnOvrCreate = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void) -> OvResult;

struct Runtime {
    _module: isize,
    session: *mut c_void,
}

// OVRPlugin functions may be invoked from any Unity thread; the session
// handle is only read behind this OnceLock by the init path.
unsafe impl Send for Runtime {}
unsafe impl Sync for Runtime {}
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

extern "system" {
    fn LoadLibraryW(library: *const u16) -> isize;
    fn GetProcAddress(module: isize, procedure: *const c_char) -> *mut c_void;
    fn FreeLibrary(module: isize) -> i32;
    fn GetLastError() -> u32;
    fn GetModuleFileNameW(module: isize, filename: *mut u16, size: u32) -> u32;
}

/// Loads the Revive runtime positioned beside the game exe as
/// LibOVRRT64_1.dll, runs ovr_Initialize + ovr_Create, and keeps the
/// session alive for the process lifetime. Idempotent; safe to call twice.
/// Returns OVRP_SUCCESS (0) or a failure code.
static LAZY_TRIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn fake_init_enabled() -> bool {
    static LABELED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let on = std::env::var("IRIS_OVR_FAKE_INIT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if on && !LABELED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        record("fake-init: IRIS_OVR_FAKE_INIT on, reporting initialized without a session (no runtime attempt)");
    }
    on
}


/// Attempted once per process, on a separate thread: Revive's runtime can
/// block inside ovr_Initialize while it tries to reach vrserver, and that
/// must never hang Unity's GetInitialized poll loop. The poll keeps
/// returning the current flag until a session actually gets created.
fn sync_init_enabled() -> bool {
    std::env::var("IRIS_OVR_SYNC_INIT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn runtime_bringup_enabled() -> bool {
    // Runtime bring-up on by default (needed for VR session). Disable with
    // IRIS_OVR_DISABLE_RUNTIME=1. Legacy IRIS_OVR_ENABLE_RUNTIME=1 still forces on.
    if std::env::var("IRIS_OVR_ENABLE_RUNTIME")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return true;
    }
    !std::env::var("IRIS_OVR_DISABLE_RUNTIME")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn runtime_dll_candidates() -> Vec<String> {
    if let Ok(v) = std::env::var("IRIS_OVR_RUNTIME_DLL") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return vec![v];
        }
    }
    let backend = std::env::var("IRIS_OVR_RUNTIME_BACKEND").unwrap_or_default();
    match backend.to_ascii_lowercase().as_str() {
        "xr" | "openxr" => vec!["LibReviveXR64.dll".into()],
        "vr" | "openvr" => vec!["LibOVRRT64_1.dll".into()],
        _ => {
            // Auto: prefer OpenXR-backed Revive (LibReviveXR64.dll). Vendored
            // LibRevive64/OpenVR (LibOVRRT64_1.dll) has been observed to AV-crash
            // inside ovr_Initialize under Proton/SteamVR (e.g. Boneworks), and an
            // in-process crash cannot be recovered as a fallback. Force OpenVR
            // explicitly with IRIS_OVR_RUNTIME_BACKEND=vr or
            // IRIS_OVR_RUNTIME_DLL=LibOVRRT64_1.dll only for targeted testing.
            vec!["LibReviveXR64.dll".into()]
        }
    }
}

fn lazy_runtime_once()
// OVRPlugin compatibility shim: exports the Oculus Unity native
// plugin ABI (ovrp_*, 411 names). Core init path implemented;
// remaining exports are zero-stubs (never reach runtime state).
use std::ffi::{c_char, c_int, c_void};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

mod abi;
mod core;
mod exports;
mod legacy;

pub use abi::*;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Build label emitted as the first trace line so logs self-identify which
/// shim build produced them (assets/ovrplugin-shim.dll is rebuilt from this
/// repo's sources; the label here is the source of truth, then git tag).
pub const SHIM_BUILD_LABEL: &str = "ovrplugin-shim 0.1.35-abi";

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
    // Report initialized optimistically once bring-up has been requested so
    // Unity/OVRPlugin integration can progress to pose/render queries while
    // LibReviveXR runtime session creation runs asynchronously. Returning false
    // forever on init failure/race kept games on the desktop path.
    true
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
                let s = v
                    .replace('/', "\\")
                    .trim_end_matches(['\\', '/'])
                    .to_string();
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
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
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
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
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
pub extern "system" fn DllMain(
    _h: *mut std::os::raw::c_void,
    reason: u32,
    _r: *mut std::os::raw::c_void,
) -> i32 {
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

// ---- runtime-backed helpers used by core.rs ----

pub fn session() -> *mut c_void {
    RUNTIME.get().map(|r| r.session).unwrap_or(std::ptr::null_mut())
}

fn cstr(p: *const u8) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe {
        let mut n = 0usize;
        while *p.add(n) != 0 && n < 256 {
            n += 1;
        }
        String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into_owned()
    }
}

pub fn status() -> Option<OvSessionStatus> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_session_status?;
    let mut s = OvSessionStatus::default();
    let rc = unsafe { f(rt.session, &mut s) };
    if rc == 0 {
        Some(s)
    } else {
        None
    }
}

pub fn device_pose(device: i32) -> Option<OvPoseStatef> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_device_poses?;
    let mut out = OvPoseStatef::default();
    let t = predicted_time(0).unwrap_or_else(now_sec);
    let rc = unsafe { f(rt.session, &device, 1, t, &mut out) };
    if rc == 0 {
        Some(out)
    } else {
        None
    }
}

pub fn predicted_time(frame: i64) -> Option<f64> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_predicted_display_time?;
    Some(unsafe { f(rt.session, frame) })
}

pub fn time_seconds() -> Option<f64> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_time_in_seconds?;
    Some(unsafe { f() })
}

pub fn connected_controllers() -> u32 {
    match RUNTIME.get() {
        Some(rt) => rt
            .api
            .get_connected_controller_types
            .map(|f| unsafe { f(rt.session) })
            .unwrap_or(CONTROLLER_TOUCH),
        None => CONTROLLER_TOUCH,
    }
}

pub fn setting_bool(name: *const u8) -> Option<i32> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_bool?;
    Some(unsafe { f(rt.session, name, 0) })
}

pub fn set_setting_bool(name: *const u8, value: i32) -> i32 {
    match RUNTIME.get() {
        Some(rt) => rt
            .api
            .set_bool
            .map(|f| unsafe { f(rt.session, name, value) })
            .unwrap_or(0),
        None => 0,
    }
}

pub fn setting_int(name: *const u8) -> Option<i32> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_int?;
    Some(unsafe { f(rt.session, name, 0) })
}

pub fn set_setting_int(name: *const u8, value: i32) -> i32 {
    match RUNTIME.get() {
        Some(rt) => rt
            .api
            .set_int
            .map(|f| unsafe { f(rt.session, name, value) })
            .unwrap_or(0),
        None => 0,
    }
}

pub fn setting_float(name: *const u8) -> Option<f32> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_float?;
    Some(unsafe { f(rt.session, name, 0.0) })
}

pub fn set_setting_float(name: *const u8, value: f32) -> i32 {
    match RUNTIME.get() {
        Some(rt) => rt
            .api
            .set_float
            .map(|f| unsafe { f(rt.session, name, value) })
            .unwrap_or(0),
        None => 0,
    }
}

pub fn setting_string(name: *const u8) -> Option<String> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_string?;
    let p = unsafe { f(rt.session, name, b"\0".as_ptr()) };
    if p.is_null() {
        None
    } else {
        Some(cstr(p))
    }
}

pub fn tracking_origin() -> Option<i32> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_tracking_origin_type?;
    Some(unsafe { f(rt.session) })
}

pub fn set_tracking_origin(origin: i32) -> i32 {
    match RUNTIME.get() {
        Some(rt) => rt
            .api
            .set_tracking_origin_type
            .map(|f| unsafe { f(rt.session, origin) })
            .unwrap_or(0),
        None => 0,
    }
}

pub fn recenter_tracking_origin() -> i32 {
    match RUNTIME.get() {
        Some(rt) => rt
            .api
            .recenter_tracking_origin
            .map(|f| unsafe { f(rt.session) })
            .unwrap_or(0),
        None => 0,
    }
}

pub fn boundary_dimensions(kind: i32) -> Option<Vector3f> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_boundary_dimensions?;
    let mut out = Vector3f::default();
    let rc = unsafe { f(rt.session, kind, &mut out) };
    if rc == 0 {
        Some(out)
    } else {
        None
    }
}

pub fn boundary_visible() -> Option<i32> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_boundary_visible?;
    let mut out = 0i32;
    let rc = unsafe { f(rt.session, &mut out) };
    if rc == 0 {
        Some(out)
    } else {
        None
    }
}

pub fn set_boundary_visible(value: i32) -> i32 {
    match RUNTIME.get() {
        Some(rt) => rt
            .api
            .request_boundary_visible
            .map(|f| unsafe { f(rt.session, value) })
            .unwrap_or(0),
        None => 0,
    }
}

pub fn test_boundary(device: i32, kind: i32) -> Option<OvBoundaryTestResult> {
    let rt = RUNTIME.get()?;
    let f = rt.api.test_boundary?;
    let mut out = OvBoundaryTestResult::default();
    let rc = unsafe { f(rt.session, device, kind, &mut out) };
    if rc == 0 {
        Some(out)
    } else {
        None
    }
}

pub fn fov_texture_size(eye: i32, fov: OvFovPort, scale: f32) -> Option<OvSizei> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_fov_texture_size?;
    Some(unsafe { f(rt.session, eye, fov, scale) })
}

pub fn tracker_count() -> u32 {
    match RUNTIME.get() {
        Some(rt) => rt.api.get_tracker_count.map(|f| unsafe { f(rt.session) }).unwrap_or(0),
        None => 0,
    }
}

pub fn runtime_version() -> Option<String> {
    let rt = RUNTIME.get()?;
    let f = rt.api.get_version_string?;
    let p = unsafe { f() };
    if p.is_null() {
        None
    } else {
        Some(cstr(p))
    }
}

type OvResult = c_int;
type FnOvrInitialize = unsafe extern "C" fn(*const c_void) -> OvResult;
type FnOvrCreate = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void) -> OvResult;

struct Runtime {
    _module: isize,
    session: *mut c_void,
    api: OvrApi,
}

/// Function pointers resolved from the loaded LibOVRRT/Revive runtime. Every
/// field is optional: a backend that does not export a symbol leaves it
/// `None` and the core falls back to its simulated value.
#[derive(Default)]
pub struct OvrApi {
    pub get_session_status: Option<unsafe extern "C" fn(*mut c_void, *mut OvSessionStatus) -> i32>,
    pub get_predicted_display_time: Option<unsafe extern "C" fn(*mut c_void, i64) -> f64>,
    pub get_time_in_seconds: Option<unsafe extern "C" fn() -> f64>,
    pub get_device_poses:
        Option<unsafe extern "C" fn(*mut c_void, *const i32, i32, f64, *mut OvPoseStatef) -> i32>,
    pub get_connected_controller_types: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    pub get_bool: Option<unsafe extern "C" fn(*mut c_void, *const u8, i32) -> i32>,
    pub set_bool: Option<unsafe extern "C" fn(*mut c_void, *const u8, i32) -> i32>,
    pub get_int: Option<unsafe extern "C" fn(*mut c_void, *const u8, i32) -> i32>,
    pub set_int: Option<unsafe extern "C" fn(*mut c_void, *const u8, i32) -> i32>,
    pub get_float: Option<unsafe extern "C" fn(*mut c_void, *const u8, f32) -> f32>,
    pub set_float: Option<unsafe extern "C" fn(*mut c_void, *const u8, f32) -> i32>,
    pub get_string: Option<unsafe extern "C" fn(*mut c_void, *const u8, *const u8) -> *const u8>,
    pub set_string: Option<unsafe extern "C" fn(*mut c_void, *const u8, *const u8, *const u8) -> i32>,
    pub get_tracking_origin_type: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub set_tracking_origin_type: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    pub recenter_tracking_origin: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub get_boundary_dimensions:
        Option<unsafe extern "C" fn(*mut c_void, i32, *mut Vector3f) -> i32>,
    pub get_boundary_visible: Option<unsafe extern "C" fn(*mut c_void, *mut i32) -> i32>,
    pub test_boundary: Option<
        unsafe extern "C" fn(*mut c_void, i32, i32, *mut OvBoundaryTestResult) -> i32,
    >,
    pub request_boundary_visible: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    pub get_fov_texture_size:
        Option<unsafe extern "C" fn(*mut c_void, i32, OvFovPort, f32) -> OvSizei>,
    pub get_tracker_count: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    pub get_version_string: Option<unsafe extern "C" fn() -> *const u8>,
    pub get_initialized: Option<unsafe extern "C" fn() -> i32>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
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

fn sync_init_enabled() -> bool {
    std::env::var("IRIS_OVR_SYNC_INIT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Attempted once per process, on a separate thread: Revive's runtime can
/// block inside ovr_Initialize while it tries to reach vrserver, and that
/// must never hang Unity's GetInitialized poll loop. The poll keeps
/// returning the current flag until a session actually gets created.

fn runtime_dll_path() -> Option<std::path::PathBuf> {
    std::env::current_exe().ok().and_then(|exe| {
        let dir = exe.parent()?;
        let p = dir.join("LibOVRRT64_1.dll");
        if p.exists() {
            Some(p)
        } else {
            None
        }
    })
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

fn lazy_runtime_once() {
    if LAZY_TRIED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if !runtime_bringup_enabled() {
        record("lazy-runtime: skipped (IRIS_OVR_DISABLE_RUNTIME=1 set; runtime bring-up disabled)");
        return;
    }
    // Always run Revive/runtime bring-up off the caller thread: LoadLibrary and
    // ovr_Initialize/ovr_Create can block on vrclient/SteamVR and must not stall
    // Unity's GetInitialized poll.
    if sync_init_enabled() {
        record("lazy-runtime: IRIS_OVR_SYNC_INIT requested but forcing threaded init to avoid blocking the game thread");
    }
    record(&format!("lazy-runtime: runtime bring-up starting (backend={}, disable with IRIS_OVR_DISABLE_RUNTIME=1)", std::env::var("IRIS_OVR_RUNTIME_BACKEND").unwrap_or_else(|_| "auto".into())));
    std::thread::spawn(|| {
        let rc = compat_initialize();
        record(&format!("lazy-runtime: compat_initialize rc {rc}"));
    });
}

fn exe_dir_wide() -> Option<Vec<u16>> {
    let mut buf = vec![0u16; 32768];
    let len = unsafe { GetModuleFileNameW(0, buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if len == 0 || len >= buf.len() {
        return None;
    }
    buf.truncate(len);
    while let Some(&c) = buf.last() {
        if c == b'\\' as u16 || c == b'/' as u16 {
            buf.pop();
            break;
        }
        buf.pop();
    }
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

fn try_load_runtime_dll(dll: &str) -> isize {
    let module = unsafe {
        let name: Vec<u16> = format!("{dll}\0").encode_utf16().collect();
        LoadLibraryW(name.as_ptr())
    };
    if module != 0 {
        record(&format!(
            "compat_initialize: loaded {dll} by name -> {module:#x}"
        ));
        return module;
    }
    let err = unsafe { GetLastError() };
    record(&format!(
        "compat_initialize: {dll} not loaded by name (GetLastError={err})"
    ));
    if let Some(mut dir) = exe_dir_wide() {
        let dir_str = String::from_utf16_lossy(&dir);
        dir.push(b'\\' as u16);
        dir.extend(format!("{dll}").encode_utf16());
        dir.push(0);
        let module = unsafe { LoadLibraryW(dir.as_ptr()) };
        if module != 0 {
            record(&format!(
                "compat_initialize: loaded {dll} from exe dir {dir_str} -> {module:#x}"
            ));
            return module;
        }
        let err2 = unsafe { GetLastError() };
        record(&format!(
            "compat_initialize: {dll} not loaded from exe dir {dir_str} (GetLastError={err2})"
        ));
    } else {
        record(&format!(
            "compat_initialize: could not resolve exe dir for {dll}"
        ));
    }
    0
}

fn proc<T>(module: isize, name: &str) -> Option<T> {
    unsafe {
        let p = GetProcAddress(module, format!("{name}\0").as_ptr() as *const c_char);
        if p.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&p))
        }
    }
}

fn load_api(module: isize) -> OvrApi {
    OvrApi {
        get_session_status: proc(module, "ovr_GetSessionStatus"),
        get_predicted_display_time: proc(module, "ovr_GetPredictedDisplayTime"),
        get_time_in_seconds: proc(module, "ovr_GetTimeInSeconds"),
        get_device_poses: proc(module, "ovr_GetDevicePoses"),
        get_connected_controller_types: proc(module, "ovr_GetConnectedControllerTypes"),
        get_bool: proc(module, "ovr_GetBool"),
        set_bool: proc(module, "ovr_SetBool"),
        get_int: proc(module, "ovr_GetInt"),
        set_int: proc(module, "ovr_SetInt"),
        get_float: proc(module, "ovr_GetFloat"),
        set_float: proc(module, "ovr_SetFloat"),
        get_string: proc(module, "ovr_GetString"),
        set_string: proc(module, "ovr_SetString"),
        get_tracking_origin_type: proc(module, "ovr_GetTrackingOriginType"),
        set_tracking_origin_type: proc(module, "ovr_SetTrackingOriginType"),
        recenter_tracking_origin: proc(module, "ovr_RecenterTrackingOrigin"),
        get_boundary_dimensions: proc(module, "ovr_GetBoundaryDimensions"),
        get_boundary_visible: proc(module, "ovr_GetBoundaryVisible"),
        test_boundary: proc(module, "ovr_TestBoundary"),
        request_boundary_visible: proc(module, "ovr_RequestBoundaryVisible"),
        get_fov_texture_size: proc(module, "ovr_GetFovTextureSize"),
        get_tracker_count: proc(module, "ovr_GetTrackerCount"),
        get_version_string: proc(module, "ovr_GetVersionString"),
        get_initialized: proc(module, "ovr_GetInitialized"),
        destroy: proc(module, "ovr_Destroy"),
    }
}

pub fn compat_initialize() -> i32 {
    if RUNTIME.get().is_some() {
        INITIALIZED.store(true, Ordering::Relaxed);
        return 0;
    }
    let mut last_rc = -1003;
    for dll in runtime_dll_candidates() {
        record(&format!("lazy-runtime: loading {dll}"));
        let module = try_load_runtime_dll(&dll);
        if module == 0 {
            last_rc = -1003;
            continue;
        }
        let raw_init =
            unsafe { GetProcAddress(module, b"ovr_Initialize\0".as_ptr() as *const c_char) };
        let raw_create =
            unsafe { GetProcAddress(module, b"ovr_Create\0".as_ptr() as *const c_char) };
        if raw_init.is_null() || raw_create.is_null() {
            let err = unsafe { GetLastError() };
            record(&format!("compat_initialize: {dll} missing ovr_Initialize or ovr_Create (GetLastError={err})"));
            unsafe {
                let _ = FreeLibrary(module);
            }
            last_rc = -1003;
            continue;
        }
        let ovr_initialize: FnOvrInitialize = unsafe { std::mem::transmute(raw_init) };
        let ovr_create: FnOvrCreate = unsafe { std::mem::transmute(raw_create) };
        record(&format!(
            "compat_initialize: calling ovr_Initialize ({dll})"
        ));
        let init_rc = unsafe { ovr_initialize(std::ptr::null()) };
        record(&format!(
            "compat_initialize: ovr_Initialize rc {init_rc} ({dll})"
        ));
        if init_rc != 0 {
            unsafe {
                let _ = FreeLibrary(module);
            }
            last_rc = -1003;
            continue;
        }
        let mut session: *mut c_void = std::ptr::null_mut();
        let mut hmd: *mut c_void = std::ptr::null_mut();
        record(&format!("compat_initialize: calling ovr_Create ({dll})"));
        let create_rc = unsafe { ovr_create(&mut session, &mut hmd) };
        record(&format!(
            "compat_initialize: ovr_Create rc {create_rc} session_is_null={} ({dll})",
            session.is_null()
        ));
        if create_rc != 0 || session.is_null() {
            unsafe {
                let _ = FreeLibrary(module);
            }
            last_rc = -1000;
            continue;
        }
        record(&format!("compat_initialize: session created ({dll})"));
        let api = load_api(module);
        let _ = RUNTIME.set(Runtime {
            _module: module,
            session,
            api,
        });
        INITIALIZED.store(true, Ordering::Relaxed);
        return 0;
    }
    last_rc
}

// ---- init-path stubs with real descriptors ----
//
// Unity 2018.4's OVRPlugin wrapper rejects the plugin (and logs
// "Security error. Oculus plugin is invalid!") when early calls return
// zeros or nulls, then crashes dereferencing a null somewhere in
// UnityPlayer. The HMD/Eye getters below return the fixed values the
// pre-init path probes, sized for a Quest-2-class device. Extra probe
// calls are traced once each so the trace names the exact rejection
// point.
//
// HmdDesc (Quest 2 class): { resolution 1832x1920, pos 0, fov 0.92/1.22 }
// EyeDesc  (per eye):      { texture 1832x1920, FOV from HmdDesc }
// Sizes are legible in the trace; bump PROBE_FOV if apps mis-size.

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct HmdDesc {
    pub resolution_w: u32,
    pub resolution_h: u32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub fov_x: f32,
    pub fov_y: f32,
    pub texture_size_w: u32,
    pub texture_size_h: u32,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct EyeDesc {
    pub resolution_w: u32,
    pub resolution_h: u32,
    pub fov_left: f32,
    pub fov_right: f32,
    pub fov_up: f32,
    pub fov_down: f32,
}

pub fn get_hmd_desc(out: *mut HmdDesc) -> i32 {
    record("get_hmd_desc (zeroed)");
    if out.is_null() {
        return 0;
    }
    unsafe {
        std::ptr::write(
            out,
            HmdDesc {
                resolution_w: 1832,
                resolution_h: 1920,
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                fov_x: 0.920,
                fov_y: 1.220,
                texture_size_w: 1832,
                texture_size_h: 1920,
            },
        );
    }
    1
}

pub fn get_eye_desc(out: *mut EyeDesc) -> i32 {
    record("get_eye_desc (zeroed)");
    if out.is_null() {
        return 0;
    }
    unsafe {
        std::ptr::write(
            out,
            EyeDesc {
                resolution_w: 1832,
                resolution_h: 1920,
                fov_left: 0.460,
                fov_right: 0.460,
                fov_up: 0.610,
                fov_down: 0.610,
            },
        );
    }
    1
}

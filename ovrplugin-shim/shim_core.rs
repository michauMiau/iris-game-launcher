use std::ffi::{c_char, c_int, c_void};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Build label emitted as the first trace line so logs self-identify which
/// shim build produced them (assets/ovrplugin-shim.dll is rebuilt from this
/// repo's sources; the label here is the source of truth, then git tag).
pub const SHIM_BUILD_LABEL: &str = "ovrplugin-shim 0.1.26-abi";

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

fn lazy_runtime_once() {
    if LAZY_TRIED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    // Always run Revive/runtime bring-up off the caller thread: LoadLibrary and
    // ovr_Initialize/ovr_Create can block on vrclient/SteamVR and must not stall
    // Unity's GetInitialized poll (seen hanging with IRIS_OVR_SYNC_INIT).
    if sync_init_enabled() {
        record("lazy-runtime: IRIS_OVR_SYNC_INIT requested but forcing threaded init to avoid blocking the game thread");
    } else {
        record("lazy-runtime: attempting LibOVRRT64_1.dll beside the exe (threaded)");
    }
    std::thread::spawn(|| {
        let rc = compat_initialize();
        record(&format!("lazy-runtime: compat_initialize rc {rc}"));
    });
}

pub fn compat_initialize() -> i32 {
    if RUNTIME.get().is_some() {
        INITIALIZED.store(true, Ordering::Relaxed);
        return 0;
    }
    let dll = "LibOVRRT64_1.dll";
    record(&format!("lazy-runtime: loading {dll}"));
    let module = unsafe {
        let name: Vec<u16> = format!("{dll}\0").encode_utf16().collect();
        LoadLibraryW(name.as_ptr())
    };
    if module == 0 {
        let err = unsafe { GetLastError() };
        record(&format!("compat_initialize: LibOVRRT64_1.dll not found/load failed (GetLastError={err})"));
        return -1003; // ovrError_InvalidOperation: no runtime found
    }
    record(&format!("compat_initialize: module loaded {module:#x}"));
    let raw_init = unsafe {
        GetProcAddress(module, b"ovr_Initialize\0".as_ptr() as *const c_char)
    };
    let raw_create = unsafe {
        GetProcAddress(module, b"ovr_Create\0".as_ptr() as *const c_char)
    };
    if raw_init.is_null() || raw_create.is_null() {
        let err = unsafe { GetLastError() };
        record(&format!("compat_initialize: missing ovr_Initialize or ovr_Create (GetLastError={err})"));
        unsafe { let _ = FreeLibrary(module); }
        return -1003;
    }
    let ovr_initialize: FnOvrInitialize = unsafe { std::mem::transmute(raw_init) };
    let ovr_create: FnOvrCreate = unsafe { std::mem::transmute(raw_create) };
    record("compat_initialize: calling ovr_Initialize");
    let init_rc = unsafe { ovr_initialize(std::ptr::null()) };
    record(&format!("compat_initialize: ovr_Initialize rc {init_rc}"));
    if init_rc != 0 {
        unsafe { let _ = FreeLibrary(module); }
        return -1003;
    }
    let mut session: *mut c_void = std::ptr::null_mut();
    let mut hmd: *mut c_void = std::ptr::null_mut();
    record("compat_initialize: calling ovr_Create");
    let create_rc = unsafe { ovr_create(&mut session, &mut hmd) };
    record(&format!("compat_initialize: ovr_Create rc {create_rc} session_is_null={}", session.is_null()));
    if create_rc != 0 || session.is_null() {
        unsafe { let _ = FreeLibrary(module); }
        return -1000;
    }
    record("compat_initialize: session created");
    let _ = RUNTIME.set(Runtime { _module: module, session });
    INITIALIZED.store(true, Ordering::Relaxed);
    0
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
    if out.is_null() { return 0; }
    unsafe {
        std::ptr::write(out, HmdDesc {
            resolution_w: 1832,
            resolution_h: 1920,
            pos_x: 0.0, pos_y: 0.0, pos_z: 0.0,
            fov_x: 0.920, fov_y: 1.220,
            texture_size_w: 1832,
            texture_size_h: 1920,
        });
    }
    1
}

pub fn get_eye_desc(out: *mut EyeDesc) -> i32 {
    record("get_eye_desc (zeroed)");
    if out.is_null() { return 0; }
    unsafe {
        std::ptr::write(out, EyeDesc {
            resolution_w: 1832,
            resolution_h: 1920,
            fov_left: 0.460, fov_right: 0.460, fov_up: 0.610, fov_down: 0.610,
        });
    }
    1
}

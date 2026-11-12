// OVRPlugin compatibility shim: exports the Oculus Unity native
// plugin ABI (ovrp_*, 411 names). Core init path implemented;
// remaining exports are zero-stubs (never reach runtime state).
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
        if p.exists() { Some(p) } else { None }
    })
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
    if let Some(p) = runtime_dll_path() {
        record(&format!("lazy-runtime: loading {}", p.to_string_lossy()));
    } else {
        record(&format!("lazy-runtime: loading {dll} (path unresolved, relying on search)"));
    }
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

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HapticsDesc {
    pub sample_rate_hz: i32,
    pub sample_size_in_bytes: i32,
    pub minimum_safe_samples_queued: i32,
    pub minimum_buffer_samples_count: i32,
    pub optimal_buffer_samples_count: i32,
    pub maximum_buffer_samples_count: i32,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HapticsState {
    pub samples_available: i32,
    pub samples_queued: i32,
}

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Posef {
    pub orientation_x: f32,
    pub orientation_y: f32,
    pub orientation_z: f32,
    pub orientation_w: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
}

// ---- generated export stubs ----
#[export_name = "?ovrp_SetFunctionPointer@@YA?AW4ovrpResult@@W4ovrpFunctionType@@PEAX@Z"]
pub extern "C" fn ovrp_set_function_pointer_alias(a0: i32, a1: *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_set_function_pointer_alias");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_AddCustomMetadata(a0: *const u8, a1: *const u8) -> i32 {
    crate::trace_call("ovrp_AddCustomMetadata");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_AutoThreadScheduling() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame(a0: i32) -> i32 {
    crate::trace_call("ovrp_BeginFrame"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame2() -> i32 {
    crate::trace_call("ovrp_BeginFrame2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame3() -> i32 {
    crate::trace_call("ovrp_BeginFrame3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame4() -> i32 {
    crate::trace_call("ovrp_BeginFrame4");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeLayerDesc() -> i32 {
    crate::trace_call("ovrp_CalculateEyeLayerDesc");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeLayerDesc2() -> i32 {
    crate::trace_call("ovrp_CalculateEyeLayerDesc2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyePreviewRect() -> i32 {
    crate::trace_call("ovrp_CalculateEyePreviewRect");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeViewportRect() -> i32 {
    crate::trace_call("ovrp_CalculateEyeViewportRect");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateLayerDesc(a0: i32, a1: i32, a2: *mut i64) -> i32 {
    crate::trace_call("ovrp_CalculateLayerDesc");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_CloseCameraDevice(a0: i32) -> i32 {
    crate::trace_call("ovrp_CloseCameraDevice");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_ConvertPoseToCameraSpace(a0: i32, a1: *mut i64, a2: *mut i64) -> i32 {
    crate::trace_call("ovrp_ConvertPoseToCameraSpace");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyDistortionWindow() -> i32 {
    crate::trace_call("ovrp_DestroyDistortionWindow");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyDistortionWindow2() -> i32 {
    crate::trace_call("ovrp_DestroyDistortionWindow2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyEyeTexture() -> i32 {
    crate::trace_call("ovrp_DestroyEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyLayer() -> i32 {
    crate::trace_call("ovrp_DestroyLayer");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyMirrorTexture() -> i32 {
    crate::trace_call("ovrp_DestroyMirrorTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyMirrorTexture2() -> i32 {
    crate::trace_call("ovrp_DestroyMirrorTexture2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DismissHSW() -> i32 {
    crate::trace_call("ovrp_DismissHSW");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DoesCameraDeviceSupportDepth(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_DoesCameraDeviceSupportDepth");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EndEye() -> i32 {
    crate::trace_call("ovrp_EndEye");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndEye2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame(a0: i32) -> i32 {
    crate::trace_call("ovrp_EndFrame"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame2() -> i32 {
    crate::trace_call("ovrp_EndFrame2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame3() -> i32 {
    crate::trace_call("ovrp_EndFrame3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame4() -> i32 {
    crate::trace_call("ovrp_EndFrame4");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueDestroyLayer(a0: *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_EnqueueDestroyLayer");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueSetupLayer(a0: *mut i32, a1: *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_EnqueueSetupLayer");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueSetupLayer2(a0: *mut i32, a1: i32, a2: *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_EnqueueSetupLayer2");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueSubmitLayer(a0: u32, a1: *mut std::os::raw::c_void, a2: *mut std::os::raw::c_void, a3: i32, a4: i32, a5: *mut i64, a6: *mut i64, a7: i32) -> i32 {
    crate::trace_call("ovrp_EnqueueSubmitLayer");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueSubmitLayer2(a0: u32, a1: *mut std::os::raw::c_void, a2: *mut std::os::raw::c_void, a3: i32, a4: i32, a5: *mut i64, a6: *mut i64, a7: i32) -> i32 {
    crate::trace_call("ovrp_EnqueueSubmitLayer2");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnumerateAllCameraDevices() -> i32 {
    crate::trace_call("ovrp_EnumerateAllCameraDevices");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EnumerateAvailableCameraDevices() -> i32 {
    crate::trace_call("ovrp_EnumerateAvailableCameraDevices");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWAdaptiveMode() -> i32 {
    crate::trace_call("ovrp_GetASWAdaptiveMode");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWDepthScale() -> i32 {
    crate::trace_call("ovrp_GetASWDepthScale");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWEnable() -> i32 {
    crate::trace_call("ovrp_GetASWEnable");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWVelocityScale() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetActiveController() -> i32 {
    crate::trace_call("ovrp_GetActiveController"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_GetActiveController2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetActiveController2"); unsafe { if !a0.is_null() { *a0 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdapterId() -> i32 {
    crate::trace_call("ovrp_GetAdapterId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdaptiveGpuPerformanceScale() -> i32 {
    crate::trace_call("ovrp_GetAdaptiveGpuPerformanceScale");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdaptiveGpuPerformanceScale2() -> i32 {
    crate::trace_call("ovrp_GetAdaptiveGpuPerformanceScale2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppAsymmetricFov(out: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetAppAsymmetricFov");
    if !out.is_null() { unsafe { *out = 0; } }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCPUPriority() -> i32 {
    crate::trace_call("ovrp_GetAppCPUPriority");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCPUPriority2() -> i32 {
    crate::trace_call("ovrp_GetAppCPUPriority2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppChromaticCorrection() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCpuStartToGpuEndTime() -> f32 {
    crate::trace_call("ovrp_GetAppCpuStartToGpuEndTime"); 0.0f32
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCpuStartToGpuEndTime2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppFramerate() -> f32 {
    crate::trace_call("ovrp_GetAppFramerate"); 90.0f32
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppFramerate2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasInputFocus(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetAppHasInputFocus"); unsafe { if !a0.is_null() { *a0 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasSystemOverlayPresent() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasVrFocus() -> i32 {
    crate::trace_call("ovrp_GetAppHasVrFocus"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasVrFocus2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetAppHasVrFocus2"); unsafe { if !a0.is_null() { *a0 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppLatencyTimings() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetAppLatencyTimings");
   std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppLatencyTimings2() -> i32 {
    crate::trace_call("ovrp_GetAppLatencyTimings2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppMonoscopic() -> i32 {
    crate::trace_call("ovrp_GetAppMonoscopic");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppPerfStats() -> i32 {
    crate::trace_call("ovrp_GetAppPerfStats");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppPerfStats2() -> i32 {
    crate::trace_call("ovrp_GetAppPerfStats2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppSRGB() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldQuit() -> i32 {
    crate::trace_call("ovrp_GetAppShouldQuit"); 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldQuit2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetAppShouldQuit2"); unsafe { if !a0.is_null() { *a0 = 0; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecenter() -> i32 {
    crate::trace_call("ovrp_GetAppShouldRecenter");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecenter2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetAppShouldRecenter2");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecreateDistortionWindow() -> i32 {
    crate::trace_call("ovrp_GetAppShouldRecreateDistortionWindow");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecreateDistortionWindow2() -> i32 {
    crate::trace_call("ovrp_GetAppShouldRecreateDistortionWindow2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInDeviceId() -> i32 {
    crate::trace_call("ovrp_GetAudioInDeviceId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInDeviceId2() -> i32 {
    crate::trace_call("ovrp_GetAudioInDeviceId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInId() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetAudioInId");
   std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInId2() -> i32 {
    crate::trace_call("ovrp_GetAudioInId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutDeviceId() -> i32 {
    crate::trace_call("ovrp_GetAudioOutDeviceId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutDeviceId2() -> i32 {
    crate::trace_call("ovrp_GetAudioOutDeviceId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutId() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetAudioOutId");
   std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutId2() -> i32 {
    crate::trace_call("ovrp_GetAudioOutId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBatteryStatus() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryConfigured() -> i32 {
    crate::trace_call("ovrp_GetBoundaryConfigured"); 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryConfigured2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetBoundaryConfigured2"); unsafe { if !a0.is_null() { *a0 = 0; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryDimensions(a0: i32) -> i64 {
    crate::trace_call("ovrp_GetBoundaryDimensions");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryDimensions2() -> i32 {
    crate::trace_call("ovrp_GetBoundaryDimensions2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryGeometry(a0: i32) -> i32 {
    crate::trace_call("ovrp_GetBoundaryGeometry");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryGeometry2(a0: i32, a1: *mut std::os::raw::c_void, a2: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetBoundaryGeometry2");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryGeometry3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryVisible() -> i32 {
    crate::trace_call("ovrp_GetBoundaryVisible"); 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryVisible2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetBoundaryVisible2"); unsafe { if !a0.is_null() { *a0 = 0; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBufferCount() -> i32 {
    crate::trace_call("ovrp_GetBufferCount");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceColorFrameBgraPixels(a0: i32, a1: *mut *mut std::os::raw::c_void, a2: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetCameraDeviceColorFrameBgraPixels");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceColorFrameSize(a0: i32, a1: *mut i64) -> i32 {
    crate::trace_call("ovrp_GetCameraDeviceColorFrameSize");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceDepthConfidencePixels(a0: i32, a1: *mut *mut std::os::raw::c_void, a2: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetCameraDeviceDepthConfidencePixels");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceDepthFramePixels(a0: i32, a1: *mut *mut std::os::raw::c_void, a2: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetCameraDeviceDepthFramePixels");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceDepthFrameSize(a0: i32, a1: *mut i64) -> i32 {
    crate::trace_call("ovrp_GetCameraDeviceDepthFrameSize");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceDepthSensingMode(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetCameraDeviceDepthSensingMode");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceIntrinsicsParameters(a0: i32, a1: *mut i32, a2: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetCameraDeviceIntrinsicsParameters");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDevicePreferredDepthQuality(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetCameraDevicePreferredDepthQuality");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCaps() -> i32 {
    crate::trace_call("ovrp_GetCaps");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCaps2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetConnectedControllers() -> i32 {
    crate::trace_call("ovrp_GetConnectedControllers"); 3
}

#[no_mangle]
pub extern "C" fn ovrp_GetConnectedControllers2() -> i32 {
    crate::trace_call("ovrp_GetConnectedControllers2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsDesc(_controller_mask: u32) -> HapticsDesc {
    crate::trace_call("ovrp_GetControllerHapticsDesc");
    HapticsDesc {
        sample_rate_hz: 320,
        sample_size_in_bytes: 1,
        minimum_safe_samples_queued: 4,
        minimum_buffer_samples_count: 16,
        optimal_buffer_samples_count: 32,
        maximum_buffer_samples_count: 64,
    }
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsDesc2() -> i32 {
    crate::trace_call("ovrp_GetControllerHapticsDesc2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsState(_controller_mask: u32) -> HapticsState {
    crate::trace_call("ovrp_GetControllerHapticsState");
    HapticsState { samples_available: 64, samples_queued: 0 }
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsState2() -> i32 {
    crate::trace_call("ovrp_GetControllerHapticsState2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState(a0: u32) -> i32 {
    crate::trace_call("ovrp_GetControllerState");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState2(a0: u32) -> i32 {
    crate::trace_call("ovrp_GetControllerState2");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState4(a0: u32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetControllerState4"); 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCurrentTrackingTransformPose(a0: *mut i64) -> i32 {
    crate::trace_call("ovrp_GetCurrentTrackingTransformPose");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDepthCompositingSupported() -> i32 {
    crate::trace_call("ovrp_GetDepthCompositingSupported");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDesiredEyeTextureFormat() -> i32 {
    crate::trace_call("ovrp_GetDesiredEyeTextureFormat");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDeviceExtensionsVk() -> i32 {
    crate::trace_call("ovrp_GetDeviceExtensionsVk");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDisplayAdapterId() -> i32 {
    crate::trace_call("ovrp_GetDisplayAdapterId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDisplayAdapterId2() -> i32 {
    crate::trace_call("ovrp_GetDisplayAdapterId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDominantHand(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetDominantHand");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraCalibrationRawPose(a0: i32, a1: *mut i64) -> i32 {
    crate::trace_call("ovrp_GetExternalCameraCalibrationRawPose");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraCount(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetExternalCameraCount");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraExtrinsics(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetExternalCameraExtrinsics");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraIntrinsics(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetExternalCameraIntrinsics");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraName() -> i32 {
    crate::trace_call("ovrp_GetExternalCameraName");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraPose(a0: i32, a1: *mut i64) -> i32 {
    crate::trace_call("ovrp_GetExternalCameraPose");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeAcceleration() -> i32 {
    crate::trace_call("ovrp_GetEyeAcceleration");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeFovLayerId() -> i32 {
    crate::trace_call("ovrp_GetEyeFovLayerId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeFrustum() -> i32 {
    crate::trace_call("ovrp_GetEyeFrustum");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeOcclusionMesh() -> i32 {
    crate::trace_call("ovrp_GetEyeOcclusionMesh");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeOcclusionMeshEnabled() -> i32 {
    crate::trace_call("ovrp_GetEyeOcclusionMeshEnabled");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePixelsPerTanAngleAtCenter() -> i32 {
    crate::trace_call("ovrp_GetEyePixelsPerTanAngleAtCenter");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePixelsPerTanAngleAtCenter2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePose(ret: *mut u8, _eye: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetEyePose"); unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 28); let p = ret as *mut f32; p.add(3).write(1.0); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePreviewRect() -> i32 {
    crate::trace_call("ovrp_GetEyePreviewRect");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeRecommendedResolutionScale() -> f32 {
    crate::trace_call("ovrp_GetEyeRecommendedResolutionScale");
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureArrayEnabled() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureArrayEnabled");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureArraySupported() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureArraySupported");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureArraySupported2() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureArraySupported2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureFlippedY() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureFlippedY");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureScale() -> f32 {
    crate::trace_call("ovrp_GetEyeTextureScale");
    1.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureShared() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureSize(_eye: i32) -> i64 {
    crate::trace_call("ovrp_GetEyeTextureSize"); if crate::initialized() { (2048i64 | (2048i64 << 32)) } else { 0i64 }
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureStageCount() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureStageCount");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeVelocity() -> i32 {
    crate::trace_call("ovrp_GetEyeVelocity");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeViewportScale() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetFloat(a0: i32) -> f32 {
    crate::trace_call("ovrp_GetFloat"); 0.0f32
}

#[no_mangle]
pub extern "C" fn ovrp_GetGPUFrameTime() -> i32 {
    crate::trace_call("ovrp_GetGPUFrameTime");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetGPUUtilLevel(a0: *mut f32) -> i32 {
    crate::trace_call("ovrp_GetGPUUtilLevel");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetGPUUtilSupported(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetGPUUtilSupported");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHandNodePoseStateLatency(a0: *mut f64) -> i32 {
    crate::trace_call("ovrp_GetHandNodePoseStateLatency");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHeadPoseModifier(a0: *mut i32, a1: *mut i64) -> i32 {
    crate::trace_call("ovrp_GetHeadPoseModifier");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHeadphonesPresent() -> i32 {
    crate::trace_call("ovrp_GetHeadphonesPresent");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHmdToEyeOffset() -> i32 {
    crate::trace_call("ovrp_GetHmdToEyeOffset");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHmdToEyeOffset2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetInitialized() -> i32 {
    crate::trace_call("ovrp_GetInitialized"); crate::initialized() as i32
}

#[no_mangle]
pub extern "C" fn ovrp_GetInputState() -> i32 {
    crate::trace_call("ovrp_GetInputState");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetInstanceExtensionsVk() -> i32 {
    crate::trace_call("ovrp_GetInstanceExtensionsVk");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerAndroidSurfaceObject(a0: i32, a1: *mut *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_GetLayerAndroidSurfaceObject");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerOcclusionMesh() -> i32 {
    crate::trace_call("ovrp_GetLayerOcclusionMesh");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTexture() -> i32 {
    crate::trace_call("ovrp_GetLayerTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTexture2() -> i32 {
    crate::trace_call("ovrp_GetLayerTexture2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTextureFoveation() -> i32 {
    crate::trace_call("ovrp_GetLayerTextureFoveation");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTexturePtr(a0: i32, a1: i32, a2: i32, a3: *mut *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_GetLayerTexturePtr");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTextureStageCount(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetLayerTextureStageCount");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetMixedRealityInitialized() -> i32 {
    crate::trace_call("ovrp_GetMixedRealityInitialized");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativePointer() -> i32 {
    crate::trace_call("ovrp_GetNativePointer");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKPointer() -> i32 {
    crate::trace_call("ovrp_GetNativeSDKPointer");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKPointer2(a0: *mut *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_GetNativeSDKPointer2");
    unsafe { if !a0.is_null() { *a0 = std::ptr::null_mut(); } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKVersion() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetNativeSDKVersion"); static V: &[u8] = b"1.43.0\0"; V.as_ptr() as *mut std::os::raw::c_void
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKVersion2(a0: *mut *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_GetNativeSDKVersion2"); unsafe { if !a0.is_null() { static V: &[u8] = b"1.43.0\0"; *a0 = V.as_ptr() as *mut std::os::raw::c_void; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeAcceleration(ret: *mut u8, _node: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetNodeAcceleration");
    unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 28); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeAcceleration2(ret: *mut u8, _state: i32, _node: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetNodeAcceleration2");
    unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 28); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeFrustum(a0: i32) -> i64 {
    crate::trace_call("ovrp_GetNodeFrustum");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeFrustum2(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeOrientationTracked(a0: i32) -> i32 {
    crate::trace_call("ovrp_GetNodeOrientationTracked"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeOrientationTracked2(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetNodeOrientationTracked2"); unsafe { if !a1.is_null() { *a1 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeOrientationValid(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePose(ret: *mut u8, _node: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetNodePose"); unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 28); let p = ret as *mut f32; p.add(3).write(1.0); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePose2(ret: *mut u8, _state: i32, _node: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetNodePose2"); unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 28); let p = ret as *mut f32; p.add(3).write(1.0); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseState(ret: *mut u8, _step: i32, _node: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetNodePoseState"); unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 88); let p = ret as *mut f32; p.add(3).write(1.0); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseState2(_step: i32, _frame: i32, _node: i32, out: *mut u8) -> i32 {
    crate::trace_call("ovrp_GetNodePoseState2"); if out.is_null() { return -1001i32; } unsafe { std::ptr::write_bytes(out, 0, 88); let p = out as *mut f32; p.add(3).write(1.0); } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseState3(_step: i32, _frame: i32, _node: i32, out: *mut u8) -> i32 {
    crate::trace_call("ovrp_GetNodePoseState3"); if out.is_null() { return -1001i32; } unsafe { std::ptr::write_bytes(out, 0, 88); let p = out as *mut f32; p.add(3).write(1.0); } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseStateRaw(_step: i32, _frame: i32, _node: i32, out: *mut u8) -> i32 {
    crate::trace_call("ovrp_GetNodePoseStateRaw"); if out.is_null() { return -1001i32; } unsafe { std::ptr::write_bytes(out, 0, 88); let p = out as *mut f32; p.add(3).write(1.0); } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePositionTracked(a0: i32) -> i32 {
    crate::trace_call("ovrp_GetNodePositionTracked"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePositionTracked2(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetNodePositionTracked2"); unsafe { if !a1.is_null() { *a1 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePositionValid(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePresent(a0: i32) -> i32 {
    crate::trace_call("ovrp_GetNodePresent"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePresent2(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetNodePresent2"); unsafe { if !a1.is_null() { *a1 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeVelocity(ret: *mut u8, _node: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetNodeVelocity");
    unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 28); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeVelocity2(ret: *mut u8, _state: i32, _node: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetNodeVelocity2");
    unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 28); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetPTWEnable() -> i32 {
    crate::trace_call("ovrp_GetPTWEnable");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPTWNear() -> i32 {
    crate::trace_call("ovrp_GetPTWNear");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPerfMetricsFloat(a0: i32, a1: *mut f32) -> i32 {
    crate::trace_call("ovrp_GetPerfMetricsFloat");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPerfMetricsInt(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetPerfMetricsInt");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPredictedDisplayTime() -> i32 {
    crate::trace_call("ovrp_GetPredictedDisplayTime");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetReorientHMDOnControllerRecenter(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetReorientHMDOnControllerRecenter");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetStatus() -> i32 {
    crate::trace_call("ovrp_GetStatus");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetStatus2() -> i32 {
    crate::trace_call("ovrp_GetStatus2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetString() -> i32 {
    crate::trace_call("ovrp_GetString");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryLevel() -> f32 {
    crate::trace_call("ovrp_GetSystemBatteryLevel");
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryLevel2() -> i32 {
    crate::trace_call("ovrp_GetSystemBatteryLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryStatus() -> i32 {
    crate::trace_call("ovrp_GetSystemBatteryStatus");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryStatus2() -> i32 {
    crate::trace_call("ovrp_GetSystemBatteryStatus2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryTemperature() -> f32 {
    crate::trace_call("ovrp_GetSystemBatteryTemperature");
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryTemperature2() -> i32 {
    crate::trace_call("ovrp_GetSystemBatteryTemperature2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemCpuLevel() -> i32 {
    crate::trace_call("ovrp_GetSystemCpuLevel");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemCpuLevel2() -> i32 {
    crate::trace_call("ovrp_GetSystemCpuLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemDisplayAvailableFrequencies(a0: *mut std::os::raw::c_void, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetSystemDisplayAvailableFrequencies");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemDisplayFrequency() -> f32 {
    crate::trace_call("ovrp_GetSystemDisplayFrequency");
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemDisplayFrequency2(out: *mut f32) -> i32 {
    crate::trace_call("ovrp_GetSystemDisplayFrequency2");
    if !out.is_null() { unsafe { *out = 90.0; } }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemGpuLevel() -> i32 {
    crate::trace_call("ovrp_GetSystemGpuLevel");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemGpuLevel2() -> i32 {
    crate::trace_call("ovrp_GetSystemGpuLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadphonesPresent() -> i32 {
    crate::trace_call("ovrp_GetSystemHeadphonesPresent");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadphonesPresent2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadsetType() -> i32 {
    crate::trace_call("ovrp_GetSystemHeadsetType"); 8
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadsetType2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetSystemHeadsetType2"); unsafe { if !a0.is_null() { *a0 = 8; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemMultiViewSupported() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemMultiViewSupported2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetSystemMultiViewSupported2"); unsafe { if !a0.is_null() { *a0 = 0; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemPowerSavingMode() -> i32 {
    crate::trace_call("ovrp_GetSystemPowerSavingMode");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemPowerSavingMode2() -> i32 {
    crate::trace_call("ovrp_GetSystemPowerSavingMode2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemProductName() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetSystemProductName");
    static V: &[u8] = b"Quest 2\0"; V.as_ptr() as *mut std::os::raw::c_void
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemProductName2() -> i32 {
    crate::trace_call("ovrp_GetSystemProductName2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRecommendedMSAALevel() -> i32 {
    crate::trace_call("ovrp_GetSystemRecommendedMSAALevel");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRecommendedMSAALevel2() -> i32 {
    crate::trace_call("ovrp_GetSystemRecommendedMSAALevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRegion() -> i32 {
    crate::trace_call("ovrp_GetSystemRegion");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRegion2() -> i32 {
    crate::trace_call("ovrp_GetSystemRegion2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVSyncCount() -> i32 {
    crate::trace_call("ovrp_GetSystemVSyncCount");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVSyncCount2() -> i32 {
    crate::trace_call("ovrp_GetSystemVSyncCount2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVolume() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVolume2(a0: *mut f32) -> i32 {
    crate::trace_call("ovrp_GetSystemVolume2"); unsafe { if !a0.is_null() { *a0 = 1.0; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTiledMultiResLevel(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetTiledMultiResLevel");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTiledMultiResSupported(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTimeInSeconds(a0: *mut f64) -> i32 {
    crate::trace_call("ovrp_GetTimeInSeconds"); unsafe { if !a0.is_null() { *a0 = crate::now_sec(); } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackerFrustum() -> i32 {
    crate::trace_call("ovrp_GetTrackerFrustum");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackerPose() -> i32 {
    crate::trace_call("ovrp_GetTrackerPose");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingCalibratedOrigin(ret: *mut u8) -> *mut u8 {
    crate::trace_call("ovrp_GetTrackingCalibratedOrigin"); unsafe { if !ret.is_null() { std::ptr::write_bytes(ret, 0, 28); let p = ret as *mut f32; p.add(3).write(1.0); } } ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingCalibratedOrigin2() -> i32 {
    crate::trace_call("ovrp_GetTrackingCalibratedOrigin2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingIPDEnabled() -> i32 {
    crate::trace_call("ovrp_GetTrackingIPDEnabled");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingIPDEnabled2() -> i32 {
    crate::trace_call("ovrp_GetTrackingIPDEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationEnabled() -> i32 {
    crate::trace_call("ovrp_GetTrackingOrientationEnabled");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationEnabled2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationSupported() -> i32 {
    crate::trace_call("ovrp_GetTrackingOrientationSupported"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationSupported2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetTrackingOrientationSupported2"); unsafe { if !a0.is_null() { *a0 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOriginType() -> i32 {
    crate::trace_call("ovrp_GetTrackingOriginType"); 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOriginType2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetTrackingOriginType2"); unsafe { if !a0.is_null() { *a0 = 0; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionEnabled() -> i32 {
    crate::trace_call("ovrp_GetTrackingPositionEnabled");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionEnabled2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionSupported() -> i32 {
    crate::trace_call("ovrp_GetTrackingPositionSupported"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionSupported2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetTrackingPositionSupported2"); unsafe { if !a0.is_null() { *a0 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingTransformRawPose(a0: *mut i64) -> i32 {
    crate::trace_call("ovrp_GetTrackingTransformRawPose");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingTransformRelativePose(out: *mut Posef, _tracking_origin: i32) -> i32 {
    crate::trace_call("ovrp_GetTrackingTransformRelativePose");
    if !out.is_null() {
        unsafe {
            *out = Posef {
                orientation_x: 0.0,
                orientation_y: 0.0,
                orientation_z: 0.0,
                orientation_w: 1.0,
                position_x: 0.0,
                position_y: 0.0,
                position_z: 0.0,
            };
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserEyeDepth() -> f32 {
    crate::trace_call("ovrp_GetUserEyeDepth");
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserEyeHeight() -> f32 {
    crate::trace_call("ovrp_GetUserEyeHeight");
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserEyeHeight2() -> i32 {
    crate::trace_call("ovrp_GetUserEyeHeight2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserIPD() -> f32 {
    crate::trace_call("ovrp_GetUserIPD");
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserIPD2() -> i32 {
    crate::trace_call("ovrp_GetUserIPD2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserNeckEyeDistance() -> i32 {
    crate::trace_call("ovrp_GetUserNeckEyeDistance");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserNeckEyeDistance2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserPresent() -> i32 {
    crate::trace_call("ovrp_GetUserPresent"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserPresent2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetUserPresent2"); unsafe { if !a0.is_null() { *a0 = 1; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetVersion() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetVersion"); static V: &[u8] = b"1.43.0\0"; V.as_ptr() as *mut std::os::raw::c_void
}

#[no_mangle]
pub extern "C" fn ovrp_GetVersion2(a0: *mut *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_GetVersion2"); unsafe { if !a0.is_null() { static V: &[u8] = b"1.43.0\0"; *a0 = V.as_ptr() as *mut std::os::raw::c_void; } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetViewportStencil() -> i32 {
    crate::trace_call("ovrp_GetViewportStencil");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetVrApiPropertyInt() -> i32 {
    crate::trace_call("ovrp_GetVrApiPropertyInt");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_HasCameraDeviceOpened(a0: i32) -> i32 {
    crate::trace_call("ovrp_HasCameraDeviceOpened");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_HasCameraDeviceOpened2() -> i32 {
    crate::trace_call("ovrp_HasCameraDeviceOpened2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize() -> i32 {
    crate::trace_call("ovrp_Initialize");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize2() -> i32 {
    crate::trace_call("ovrp_Initialize2");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize3() -> i32 {
    crate::trace_call("ovrp_Initialize3");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize4() -> i32 {
    crate::trace_call("ovrp_Initialize4");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize5() -> i32 {
    crate::trace_call("ovrp_Initialize5");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_InitializeMixedReality() -> i32 {
    crate::trace_call("ovrp_InitializeMixedReality");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceAvailable(a0: i32) -> i32 {
    crate::trace_call("ovrp_IsCameraDeviceAvailable");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceAvailable2() -> i32 {
    crate::trace_call("ovrp_IsCameraDeviceAvailable2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceColorFrameAvailable(a0: i32) -> i32 {
    crate::trace_call("ovrp_IsCameraDeviceColorFrameAvailable");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceColorFrameAvailable2() -> i32 {
    crate::trace_call("ovrp_IsCameraDeviceColorFrameAvailable2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceDepthFrameAvailable(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_IsCameraDeviceDepthFrameAvailable");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_IsPerfMetricsSupported(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_IsPerfMetricsSupported");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_IsRequestingASWData() -> i32 {
    crate::trace_call("ovrp_IsRequestingASWData");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_EncodeMrcFrame(a0: i32, a1: i32, a2: i32, a3: i32, a4: f64, a5: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_EncodeMrcFrame");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_EncodeMrcFrameWithDualTextures(a0: i32, a1: i32, a2: i32, a3: i32, a4: i32, a5: f64, a6: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_EncodeMrcFrameWithDualTextures");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetInitialized(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_GetInitialized");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcActivationMode(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_GetMrcActivationMode");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcAudioSampleRate(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_GetMrcAudioSampleRate");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcFrameImageFlipped(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_GetMrcFrameImageFlipped");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcFrameInverseAlpha() -> i32 {
    crate::trace_call("ovrp_Media_GetMrcFrameInverseAlpha");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcFrameSize(a0: *mut i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_GetMrcFrameSize");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcInputVideoBufferType(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_GetMrcInputVideoBufferType");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_Initialize() -> i32 {
    crate::trace_call("ovrp_Media_Initialize");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_IsMrcActivated(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_IsMrcActivated");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_IsMrcEnabled(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_IsMrcEnabled");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetAvailableQueueIndexVulkan() -> i32 {
    crate::trace_call("ovrp_Media_SetAvailableQueueIndexVulkan");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcActivationMode(a0: i32) -> i32 {
    crate::trace_call("ovrp_Media_SetMrcActivationMode");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcAudioSampleRate(a0: i32) -> i32 {
    crate::trace_call("ovrp_Media_SetMrcAudioSampleRate");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcFrameImageFlipped(a0: i32) -> i32 {
    crate::trace_call("ovrp_Media_SetMrcFrameImageFlipped");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcFrameInverseAlpha() -> i32 {
    crate::trace_call("ovrp_Media_SetMrcFrameInverseAlpha");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcFrameSize(a0: i32, a1: i32) -> i32 {
    crate::trace_call("ovrp_Media_SetMrcFrameSize");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcInputVideoBufferType(a0: i32) -> i32 {
    crate::trace_call("ovrp_Media_SetMrcInputVideoBufferType");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_Shutdown() -> i32 {
    crate::trace_call("ovrp_Media_Shutdown");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SyncMrcFrame(a0: i32) -> i32 {
    crate::trace_call("ovrp_Media_SyncMrcFrame");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_Update() -> i32 {
    crate::trace_call("ovrp_Media_Update");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_UseMrcDebugCamera(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_Media_UseMrcDebugCamera");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_OpenCameraDevice(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize() -> i32 {
    crate::trace_call("ovrp_PreInitialize"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize2() -> i32 {
    crate::trace_call("ovrp_PreInitialize2"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize3() -> i32 {
    crate::trace_call("ovrp_PreInitialize3"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_RecenterPose() -> i32 {
    crate::trace_call("ovrp_RecenterPose");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_RecenterTrackingOrigin(a0: u32) -> i32 {
    crate::trace_call("ovrp_RecenterTrackingOrigin");
    1
}

#[no_mangle]
pub extern "C" fn ovrp_RecenterTrackingOrigin2() -> i32 {
    crate::trace_call("ovrp_RecenterTrackingOrigin2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_RecreateEyeTexture() -> i32 {
    crate::trace_call("ovrp_RecreateEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ReleaseEyeTexture() -> i32 {
    crate::trace_call("ovrp_ReleaseEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ResetAppPerfStats() -> i32 {
    crate::trace_call("ovrp_ResetAppPerfStats");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_ResetAppPerfStats2() -> i32 {
    crate::trace_call("ovrp_ResetAppPerfStats2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SendEvent(a0: *const u8, a1: *const u8) -> i32 {
    crate::trace_call("ovrp_SendEvent");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SendEvent2(a0: *const u8, a1: *const u8, a2: *const u8) -> i32 {
    crate::trace_call("ovrp_SendEvent2");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetASWAdaptiveMode() -> i32 {
    crate::trace_call("ovrp_SetASWAdaptiveMode");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetASWEnable() -> i32 {
    crate::trace_call("ovrp_SetASWEnable");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppAsymmetricFov() -> i32 {
    crate::trace_call("ovrp_SetAppAsymmetricFov");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppCPUPriority() -> i32 {
    crate::trace_call("ovrp_SetAppCPUPriority");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppCPUPriority2() -> i32 {
    crate::trace_call("ovrp_SetAppCPUPriority2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppChromaticCorrection(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetAppChromaticCorrection");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppEngineInfo() -> i32 {
    crate::trace_call("ovrp_SetAppEngineInfo");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppEngineInfo2() -> i32 {
    crate::trace_call("ovrp_SetAppEngineInfo2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppIgnoreVrFocus() -> i32 {
    crate::trace_call("ovrp_SetAppIgnoreVrFocus");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppMonoscopic(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetAppMonoscopic");
    1
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppSRGB() -> i32 {
    crate::trace_call("ovrp_SetAppSRGB");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetBoundaryVisible(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetBoundaryVisible");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetBoundaryVisible2() -> i32 {
    crate::trace_call("ovrp_SetBoundaryVisible2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCameraDeviceDepthSensingMode(a0: i32, a1: i32) -> i32 {
    crate::trace_call("ovrp_SetCameraDeviceDepthSensingMode");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCameraDevicePreferredColorFrameSize(a0: i32, a1: i64) -> i32 {
    crate::trace_call("ovrp_SetCameraDevicePreferredColorFrameSize");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCameraDevicePreferredDepthQuality(a0: i32, a1: i32) -> i32 {
    crate::trace_call("ovrp_SetCameraDevicePreferredDepthQuality");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCaps() -> i32 {
    crate::trace_call("ovrp_SetCaps");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetColorScaleAndOffset(a0: i32, a1: i32, a2: i32) -> i32 {
    crate::trace_call("ovrp_SetColorScaleAndOffset");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerHaptics(a0: u32, a1: i32) -> i32 {
    crate::trace_call("ovrp_SetControllerHaptics");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerHaptics2() -> i32 {
    crate::trace_call("ovrp_SetControllerHaptics2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerVibration(a0: u32, a1: f32, a2: f32) -> i32 {
    crate::trace_call("ovrp_SetControllerVibration");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerVibration2() -> i32 {
    crate::trace_call("ovrp_SetControllerVibration2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDepthCompositingInfo() -> i32 {
    crate::trace_call("ovrp_SetDepthCompositingInfo");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDepthProjInfo() -> i32 {
    crate::trace_call("ovrp_SetDepthProjInfo");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDesiredEyeTextureFormat(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetDesiredEyeTextureFormat");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDeveloperMode(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetDeveloperMode");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeOcclusionMeshEnabled(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetEyeOcclusionMeshEnabled");
    1
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTexture() -> i32 {
    crate::trace_call("ovrp_SetEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureArrayEnabled() -> i32 {
    crate::trace_call("ovrp_SetEyeTextureArrayEnabled");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureFlippedY() -> i32 {
    crate::trace_call("ovrp_SetEyeTextureFlippedY");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureScale(a0: f32) -> i32 {
    crate::trace_call("ovrp_SetEyeTextureScale");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureShared() -> i32 {
    crate::trace_call("ovrp_SetEyeTextureShared");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeViewportScale() -> i32 {
    crate::trace_call("ovrp_SetEyeViewportScale");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetFloat() -> i32 {
    crate::trace_call("ovrp_SetFloat");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetHandNodePoseStateLatency(a0: f64) -> i32 {
    crate::trace_call("ovrp_SetHandNodePoseStateLatency");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetHeadPoseModifier(a0: *mut i32, a1: *mut i64) -> i32 {
    crate::trace_call("ovrp_SetHeadPoseModifier");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetInhibitSystemUX() -> i32 {
    crate::trace_call("ovrp_SetInhibitSystemUX");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetInhibitSystemUX2() -> i32 {
    crate::trace_call("ovrp_SetInhibitSystemUX2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetNodePositionTracked() -> i32 {
    crate::trace_call("ovrp_SetNodePositionTracked");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetNodePositionTracked2() -> i32 {
    crate::trace_call("ovrp_SetNodePositionTracked2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOctilinearInfo() -> i32 {
    crate::trace_call("ovrp_SetOctilinearInfo");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOverlayQuad() -> i32 {
    crate::trace_call("ovrp_SetOverlayQuad");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOverlayQuad2(a0: i32, a1: i32, a2: *mut std::os::raw::c_void, a3: *mut std::os::raw::c_void, a4: i64, a5: i64) -> i32 {
    crate::trace_call("ovrp_SetOverlayQuad2");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOverlayQuad3(a0: u32, a1: *mut std::os::raw::c_void, a2: *mut std::os::raw::c_void, a3: *mut std::os::raw::c_void, a4: i64, a5: i64, a6: i32) -> i32 {
    crate::trace_call("ovrp_SetOverlayQuad3");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetPTWEnable() -> i32 {
    crate::trace_call("ovrp_SetPTWEnable");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetRemoteHandedness() -> i32 {
    crate::trace_call("ovrp_SetRemoteHandedness");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetReorientHMDOnControllerRecenter(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetReorientHMDOnControllerRecenter");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemCpuLevel(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetSystemCpuLevel");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemCpuLevel2() -> i32 {
    crate::trace_call("ovrp_SetSystemCpuLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemDisplayFrequency(a0: f32) -> i32 {
    crate::trace_call("ovrp_SetSystemDisplayFrequency");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemGpuLevel(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetSystemGpuLevel");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemGpuLevel2() -> i32 {
    crate::trace_call("ovrp_SetSystemGpuLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemVSyncCount(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetSystemVSyncCount");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemVSyncCount2() -> i32 {
    crate::trace_call("ovrp_SetSystemVSyncCount2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetThreadPerformance() -> i32 {
    crate::trace_call("ovrp_SetThreadPerformance");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTiledMultiResLevel(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetTiledMultiResLevel");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingCalibratedOrigin2() -> i32 {
    crate::trace_call("ovrp_SetTrackingCalibratedOrigin2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingIPDEnabled(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetTrackingIPDEnabled");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingIPDEnabled2() -> i32 {
    crate::trace_call("ovrp_SetTrackingIPDEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOrientationEnabled(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetTrackingOrientationEnabled");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOrientationEnabled2() -> i32 {
    crate::trace_call("ovrp_SetTrackingOrientationEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOriginType(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetTrackingOriginType");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOriginType2() -> i32 {
    crate::trace_call("ovrp_SetTrackingOriginType2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingPositionEnabled(a0: i32) -> i32 {
    crate::trace_call("ovrp_SetTrackingPositionEnabled");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingPositionEnabled2() -> i32 {
    crate::trace_call("ovrp_SetTrackingPositionEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserEyeDepth(a0: f32) -> i32 {
    crate::trace_call("ovrp_SetUserEyeDepth");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserEyeHeight(a0: f32) -> i32 {
    crate::trace_call("ovrp_SetUserEyeHeight");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserEyeHeight2() -> i32 {
    crate::trace_call("ovrp_SetUserEyeHeight2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserIPD(a0: f32) -> i32 {
    crate::trace_call("ovrp_SetUserIPD");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserIPD2() -> i32 {
    crate::trace_call("ovrp_SetUserIPD2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserNeckEyeDistance() -> i32 {
    crate::trace_call("ovrp_SetUserNeckEyeDistance");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserNeckEyeDistance2() -> i32 {
    crate::trace_call("ovrp_SetUserNeckEyeDistance2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetVrApiPropertyFloat() -> i32 {
    crate::trace_call("ovrp_SetVrApiPropertyFloat");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetVrApiPropertyInt() -> i32 {
    crate::trace_call("ovrp_SetVrApiPropertyInt");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDisplayObjects() -> i32 {
    crate::trace_call("ovrp_SetupDisplayObjects");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDisplayObjects2() -> i32 {
    crate::trace_call("ovrp_SetupDisplayObjects2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow() -> i32 {
    crate::trace_call("ovrp_SetupDistortionWindow");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow2() -> i32 {
    crate::trace_call("ovrp_SetupDistortionWindow2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow3() -> i32 {
    crate::trace_call("ovrp_SetupDistortionWindow3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupEyeTexture() -> i32 {
    crate::trace_call("ovrp_SetupEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupEyeTexture2() -> i32 {
    crate::trace_call("ovrp_SetupEyeTexture2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupLayer() -> i32 {
    crate::trace_call("ovrp_SetupLayer");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupLayerDepth() -> i32 {
    crate::trace_call("ovrp_SetupLayerDepth");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupMirrorTexture() -> i32 {
    crate::trace_call("ovrp_SetupMirrorTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupMirrorTexture2() -> i32 {
    crate::trace_call("ovrp_SetupMirrorTexture2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ShowSystemUI(a0: i32) -> i32 {
    crate::trace_call("ovrp_ShowSystemUI");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_ShowSystemUI2() -> i32 {
    crate::trace_call("ovrp_ShowSystemUI2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ShowUI() -> i32 {
    crate::trace_call("ovrp_ShowUI");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Shutdown() -> i32 {
    crate::trace_call("ovrp_Shutdown");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Shutdown2() -> i32 {
    crate::trace_call("ovrp_Shutdown2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ShutdownMixedReality() -> i32 {
    crate::trace_call("ovrp_ShutdownMixedReality");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryNode(a0: i32, a1: i32) -> i32 {
    crate::trace_call("ovrp_TestBoundaryNode");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryNode2() -> i32 {
    crate::trace_call("ovrp_TestBoundaryNode2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryPoint(a0: i64, a1: i32) -> i32 {
    crate::trace_call("ovrp_TestBoundaryPoint");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryPoint2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Update(a0: i32) -> i32 {
    crate::trace_call("ovrp_Update"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_Update2(a0: i32, a1: i32, a2: f64) -> i32 {
    crate::trace_call("ovrp_Update2"); 1
}

#[no_mangle]
pub extern "C" fn ovrp_Update3(a0: i32, a1: i32, a2: f64) -> i32 {
    crate::trace_call("ovrp_Update3"); 0
}

#[no_mangle]
pub extern "C" fn ovrp_UpdateCameraDevices() -> i32 {
    crate::trace_call("ovrp_UpdateCameraDevices");
   0
}

#[no_mangle]
pub extern "C" fn ovrp_UpdateExternalCamera() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_WaitToBeginFrame(a0: i32) -> i32 {
    crate::trace_call("ovrp_WaitToBeginFrame"); 0
}

#[no_mangle]
pub extern "C" fn ovrpi_SetTrackingCalibratedOrigin() -> i32 {
    crate::trace_call("ovrpi_SetTrackingCalibratedOrigin");
    0
}

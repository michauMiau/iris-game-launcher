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
pub const SHIM_BUILD_LABEL: &str = "ovrplugin-shim 0.1.17";

pub fn initialized() -> bool {
    INITIALIZED.load(Ordering::Relaxed)
}

pub fn set_initialized(v: bool) {
    INITIALIZED.store(v, Ordering::Relaxed);
}

fn log_path() -> String {
    std::env::var("OVR_SHIM_LOG").unwrap_or_else(|_| "C:\\iris-shim-trace.log".to_string())
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
    // names the shim build so logs self-identify the place and version.
    static BUILD_LABELED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let Some(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .ok()
    else {
        return;
    };
    if !BUILD_LABELED.swap(true, std::sync::atomic::Ordering::Relaxed) {
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
}

/// Loads the Revive runtime positioned beside the game exe as
/// LibOVRRT64_1.dll, runs ovr_Initialize + ovr_Create, and keeps the
/// session alive for the process lifetime. Idempotent; safe to call twice.
/// Returns OVRP_SUCCESS (0) or a failure code.
pub fn compat_initialize() -> i32 {
    if RUNTIME.get().is_some() {
        INITIALIZED.store(true, Ordering::Relaxed);
        return 0;
    }
    let module = unsafe {
        let name: Vec<u16> = "LibOVRRT64_1.dll\0".encode_utf16().collect();
        LoadLibraryW(name.as_ptr())
    };
    if module == 0 {
        record("compat_initialize: LibOVRRT64_1.dll not found");
        return -1003; // ovrError_InvalidOperation: no runtime found
    }
    let raw_init = unsafe {
        GetProcAddress(module, b"ovr_Initialize\0".as_ptr() as *const c_char)
    };
    let raw_create = unsafe {
        GetProcAddress(module, b"ovr_Create\0".as_ptr() as *const c_char)
    };
    if raw_init.is_null() || raw_create.is_null() {
        record("compat_initialize: missing ovr_Initialize or ovr_Create");
        unsafe { let _ = FreeLibrary(module); }
        return -1003;
    }
    let ovr_initialize: FnOvrInitialize = unsafe { std::mem::transmute(raw_init) };
    let ovr_create: FnOvrCreate = unsafe { std::mem::transmute(raw_create) };
    if unsafe { ovr_initialize(std::ptr::null()) } != 0 {
        record("compat_initialize: ovr_Initialize failed");
        unsafe { let _ = FreeLibrary(module); }
        return -1003;
    }
    let mut session: *mut c_void = std::ptr::null_mut();
    let mut hmd: *mut c_void = std::ptr::null_mut();
    if unsafe { ovr_create(&mut session, &mut hmd) } != 0 || session.is_null() {
        record("compat_initialize: ovr_Create failed");
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
// ---- generated export stubs ----
#[export_name = "?ovrp_SetFunctionPointer@@YA?AW4ovrpResult@@W4ovrpFunctionType@@PEAX@Z"]
pub extern "C" fn ovrp_set_function_pointer_alias(a0: i32, a1: *mut std::os::raw::c_void) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_AddCustomMetadata(a0: *const u8, a1: *const u8) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_AutoThreadScheduling() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame() -> i32 {
    crate::begin_frame()
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame4() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeLayerDesc() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeLayerDesc2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyePreviewRect() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeViewportRect() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateLayerDesc(a0: i32, a1: i32, a2: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_CloseCameraDevice(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_ConvertPoseToCameraSpace(a0: i32, a1: *mut i32, a2: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyDistortionWindow() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyDistortionWindow2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyEyeTexture() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyLayer() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyMirrorTexture() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyMirrorTexture2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DismissHSW() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DoesCameraDeviceSupportDepth(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EndEye() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndEye2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame() -> i32 {
    crate::end_frame()
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame4() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueDestroyLayer(a0: *mut std::os::raw::c_void) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueSetupLayer(a0: *mut i32, a1: *mut std::os::raw::c_void) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueSetupLayer2(a0: *mut i32, a1: i32, a2: *mut std::os::raw::c_void) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueSubmitLayer(a0: u32, a1: *mut std::os::raw::c_void, a2: *mut std::os::raw::c_void, a3: i32, a4: i32, a5: *mut i32, a6: *mut i32, a7: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnqueueSubmitLayer2(a0: u32, a1: *mut std::os::raw::c_void, a2: *mut std::os::raw::c_void, a3: i32, a4: i32, a5: *mut i32, a6: *mut i32, a7: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_EnumerateAllCameraDevices() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EnumerateAvailableCameraDevices() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWAdaptiveMode() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWDepthScale() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWEnable() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWVelocityScale() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetActiveController() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ovrp_GetActiveController2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdapterId() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdaptiveGpuPerformanceScale() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdaptiveGpuPerformanceScale2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppAsymmetricFov(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCPUPriority() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCPUPriority2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppChromaticCorrection() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCpuStartToGpuEndTime() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCpuStartToGpuEndTime2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppFramerate() -> f32 {
    90.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppFramerate2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasInputFocus(a0: *mut i32) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasSystemOverlayPresent() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasVrFocus() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasVrFocus2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppLatencyTimings() -> *mut std::os::raw::c_void {
   std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppLatencyTimings2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppMonoscopic() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppPerfStats() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppPerfStats2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppSRGB() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldQuit() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldQuit2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecenter() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecenter2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecreateDistortionWindow() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecreateDistortionWindow2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInDeviceId() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInDeviceId2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInId() -> *mut std::os::raw::c_void {
   std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInId2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutDeviceId() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutDeviceId2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutId() -> *mut std::os::raw::c_void {
   std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutId2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBatteryStatus() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryConfigured() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryConfigured2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryDimensions(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryDimensions2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryGeometry(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryGeometry2(a0: i32, a1: *mut std::os::raw::c_void, a2: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryGeometry3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryVisible() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryVisible2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBufferCount() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceColorFrameBgraPixels(a0: i32, a1: *mut *mut std::os::raw::c_void, a2: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceColorFrameSize(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceDepthConfidencePixels(a0: i32, a1: *mut *mut std::os::raw::c_void, a2: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceDepthFramePixels(a0: i32, a1: *mut *mut std::os::raw::c_void, a2: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceDepthFrameSize(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceDepthSensingMode(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDeviceIntrinsicsParameters(a0: i32, a1: *mut i32, a2: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCameraDevicePreferredDepthQuality(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCaps() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCaps2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetConnectedControllers() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetConnectedControllers2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsDesc(a0: u32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsDesc2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsState(a0: u32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsState2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState(a0: u32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState2(a0: u32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState4(a0: u32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCurrentTrackingTransformPose(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDepthCompositingSupported() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDesiredEyeTextureFormat() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDeviceExtensionsVk() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDisplayAdapterId() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDisplayAdapterId2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDominantHand(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraCalibrationRawPose(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraCount(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraExtrinsics(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraIntrinsics(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraName() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraPose(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeAcceleration() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeFovLayerId() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeFrustum() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeOcclusionMesh() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeOcclusionMeshEnabled() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePixelsPerTanAngleAtCenter() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePixelsPerTanAngleAtCenter2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePose() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePreviewRect() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeRecommendedResolutionScale() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureArrayEnabled() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureArraySupported() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureArraySupported2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureFlippedY() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureScale() -> f32 {
    1.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureShared() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureSize(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureStageCount() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeVelocity() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeViewportScale() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetFloat() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetGPUFrameTime() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetGPUUtilLevel(a0: *mut f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetGPUUtilSupported(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHandNodePoseStateLatency(a0: *mut f64) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHeadPoseModifier(a0: *mut i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHeadphonesPresent() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHmdToEyeOffset() -> i32 {
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
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetInstanceExtensionsVk() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerAndroidSurfaceObject(a0: i32, a1: *mut *mut std::os::raw::c_void) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerOcclusionMesh() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTexture() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTexture2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTextureFoveation() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTexturePtr(a0: i32, a1: i32, a2: i32, a3: *mut *mut std::os::raw::c_void) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTextureStageCount(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetMixedRealityInitialized() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativePointer() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKPointer() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKPointer2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKVersion() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetNativeSDKVersion"); static V: &[u8] = b"1.43.0\0"; V.as_ptr() as *mut std::os::raw::c_void
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKVersion2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeAcceleration(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeAcceleration2(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeFrustum(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeFrustum2(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeOrientationTracked(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeOrientationTracked2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeOrientationValid(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePose(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePose2(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseState(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseState2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseState3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseStateRaw(a0: i32, a1: i32, a2: i32, a3: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePositionTracked(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePositionTracked2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePositionValid(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePresent(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePresent2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeVelocity(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeVelocity2(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPTWEnable() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPTWNear() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPerfMetricsFloat(a0: i32, a1: *mut f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPerfMetricsInt(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPredictedDisplayTime() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetReorientHMDOnControllerRecenter(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetStatus() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetStatus2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetString() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryLevel() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryLevel2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryStatus() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryStatus2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryTemperature() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryTemperature2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemCpuLevel() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemCpuLevel2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemDisplayAvailableFrequencies(a0: *mut std::os::raw::c_void, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemDisplayFrequency() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemDisplayFrequency2(a0: *mut f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemGpuLevel() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemGpuLevel2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadphonesPresent() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadphonesPresent2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadsetType() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadsetType2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemMultiViewSupported() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemMultiViewSupported2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemPowerSavingMode() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemPowerSavingMode2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemProductName() -> *mut std::os::raw::c_void {
    static V: &[u8] = b"Quest 2\0"; V.as_ptr() as *mut std::os::raw::c_void
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemProductName2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRecommendedMSAALevel() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRecommendedMSAALevel2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRegion() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRegion2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVSyncCount() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVSyncCount2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVolume() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVolume2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTiledMultiResLevel(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTiledMultiResSupported(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTimeInSeconds(a0: *mut f64) -> i32 {
    unsafe { if !a0.is_null() { *a0 = crate::now_sec(); } } 0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackerFrustum() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackerPose() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingCalibratedOrigin() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingCalibratedOrigin2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingIPDEnabled() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingIPDEnabled2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationEnabled() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationEnabled2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationSupported() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationSupported2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOriginType() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOriginType2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionEnabled() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionEnabled2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionSupported() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionSupported2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingTransformRawPose(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingTransformRelativePose(a0: *mut i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserEyeDepth() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserEyeHeight() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserEyeHeight2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserIPD() -> f32 {
   0.0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserIPD2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserNeckEyeDistance() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserNeckEyeDistance2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserPresent() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserPresent2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetVersion() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetVersion"); static V: &[u8] = b"1.43.0\0"; V.as_ptr() as *mut std::os::raw::c_void
}

#[no_mangle]
pub extern "C" fn ovrp_GetVersion2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetViewportStencil() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetVrApiPropertyInt() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_HasCameraDeviceOpened(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_HasCameraDeviceOpened2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize() -> i32 {
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize2() -> i32 {
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize3() -> i32 {
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize4() -> i32 {
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize5() -> i32 {
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_InitializeMixedReality() -> i32 {
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceAvailable(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceAvailable2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceColorFrameAvailable(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceColorFrameAvailable2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceDepthFrameAvailable(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_IsPerfMetricsSupported(a0: i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_IsRequestingASWData() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_EncodeMrcFrame(a0: i32, a1: i32, a2: i32, a3: i32, a4: f64, a5: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_EncodeMrcFrameWithDualTextures(a0: i32, a1: i32, a2: i32, a3: i32, a4: i32, a5: f64, a6: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetInitialized(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcActivationMode(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcAudioSampleRate(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcFrameImageFlipped(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcFrameInverseAlpha() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcFrameSize(a0: *mut i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcInputVideoBufferType(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_Initialize() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_IsMrcActivated(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_IsMrcEnabled(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetAvailableQueueIndexVulkan() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcActivationMode(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcAudioSampleRate(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcFrameImageFlipped(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcFrameInverseAlpha() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcFrameSize(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcInputVideoBufferType(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_Shutdown() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SyncMrcFrame(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_Update() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_UseMrcDebugCamera(a0: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_OpenCameraDevice(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_RecenterPose() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_RecenterTrackingOrigin(a0: u32) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ovrp_RecenterTrackingOrigin2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_RecreateEyeTexture() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ReleaseEyeTexture() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ResetAppPerfStats() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_ResetAppPerfStats2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SendEvent(a0: *const u8, a1: *const u8) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SendEvent2(a0: *const u8, a1: *const u8, a2: *const u8) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetASWAdaptiveMode() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetASWEnable() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppAsymmetricFov() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppCPUPriority() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppCPUPriority2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppChromaticCorrection(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppEngineInfo() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppEngineInfo2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppIgnoreVrFocus() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppMonoscopic(a0: i32) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppSRGB() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetBoundaryVisible(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetBoundaryVisible2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCameraDeviceDepthSensingMode(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCameraDevicePreferredColorFrameSize(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCameraDevicePreferredDepthQuality(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCaps() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetColorScaleAndOffset(a0: i32, a1: i32, a2: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerHaptics(a0: u32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerHaptics2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerVibration(a0: u32, a1: f32, a2: f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerVibration2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDepthCompositingInfo() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDepthProjInfo() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDesiredEyeTextureFormat(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDeveloperMode(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeOcclusionMeshEnabled(a0: i32) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTexture() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureArrayEnabled() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureFlippedY() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureScale(a0: f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureShared() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeViewportScale() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetFloat() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetHandNodePoseStateLatency(a0: f64) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetHeadPoseModifier(a0: *mut i32, a1: *mut i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetInhibitSystemUX() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetInhibitSystemUX2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetNodePositionTracked() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetNodePositionTracked2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOctilinearInfo() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOverlayQuad() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOverlayQuad2(a0: i32, a1: i32, a2: *mut std::os::raw::c_void, a3: *mut std::os::raw::c_void, a4: i32, a5: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOverlayQuad3(a0: u32, a1: *mut std::os::raw::c_void, a2: *mut std::os::raw::c_void, a3: *mut std::os::raw::c_void, a4: i32, a5: i32, a6: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetPTWEnable() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetRemoteHandedness() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetReorientHMDOnControllerRecenter(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemCpuLevel(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemCpuLevel2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemDisplayFrequency(a0: f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemGpuLevel(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemGpuLevel2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemVSyncCount(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemVSyncCount2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetThreadPerformance() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTiledMultiResLevel(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingCalibratedOrigin2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingIPDEnabled(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingIPDEnabled2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOrientationEnabled(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOrientationEnabled2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOriginType(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOriginType2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingPositionEnabled(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingPositionEnabled2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserEyeDepth(a0: f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserEyeHeight(a0: f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserEyeHeight2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserIPD(a0: f32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserIPD2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserNeckEyeDistance() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserNeckEyeDistance2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetVrApiPropertyFloat() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetVrApiPropertyInt() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDisplayObjects() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDisplayObjects2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupEyeTexture() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupEyeTexture2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupLayer() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupLayerDepth() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupMirrorTexture() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupMirrorTexture2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ShowSystemUI(a0: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_ShowSystemUI2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ShowUI() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Shutdown() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Shutdown2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ShutdownMixedReality() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryNode(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryNode2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryPoint(a0: i32, a1: i32) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryPoint2() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Update() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Update2(a0: i32, a1: i32, a2: f64) -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_Update3() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrp_UpdateCameraDevices() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_UpdateExternalCamera() -> i32 {
   0
}

#[no_mangle]
pub extern "C" fn ovrp_WaitToBeginFrame() -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn ovrpi_SetTrackingCalibratedOrigin() -> i32 {
    0
}

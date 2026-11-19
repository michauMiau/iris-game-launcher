include!(concat!(env!("OUT_DIR"), "/generated.rs"));

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::collections::VecDeque;

/// Typed message/result object handed to the Unity wrapper. Same layout works
/// as both Message and Completed result.
pub struct Msg {
    pub msg_type: u32,
    pub is_error: bool,
    pub entitled: bool,
    /// Request id the SDK matches a message against (ovr_Message_GetRequestID).
    pub id: u32,
}

/// ovrMessage_Entitlement_GetIsViewerEntitled from OVR_MessageType.h
const MSG_ENTITLEMENT: u32 = 0x186B58B1;

/// Stable non-NULL handle for unknown getters: one zeroed static record, so
/// retained handles never get recycled.
static RECORD: [u8; 128] = [0; 128];

/// Static empty C string handed back by string getters. The SDK reads or logs
/// these, and a zero-length string never crashes it.
static EMPTY: [u8; 2] = [0, 0];

fn empty_str() -> *mut u8 {
    &EMPTY as *const [u8; 2] as *mut u8
}

/// Read a C string argument (null pointer reads as empty), capped short.
fn cstr(ptr: *const u8) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut bytes = Vec::new();
    unsafe {
        for i in 0..160 {
            let c = *ptr.add(i);
            if c == 0 {
                break;
            }
            bytes.push(c);
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn handle() -> *mut u8 {
    &RECORD as *const [u8; 128] as *mut u8
}

fn queue() -> &'static Mutex<VecDeque<Box<Msg>>> {
    static QUEUE: OnceLock<Mutex<VecDeque<Box<Msg>>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
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

/// Polling noise is suppressed: a null PopMessage and a FreeMessage of a null
/// message do nothing, and Bonelab polled 212k times per session with those.
/// Null-poll heartbeat: the games poll PopMessage forever while waiting, and
/// suppressing every null hid whether the game thread was alive or wedged.
/// One line per 200 idle polls keeps the timeline small but proves liveness.
static NULL_POLLS: AtomicU32 = AtomicU32::new(0);

fn record_pop(msg: *mut Msg) {
    if msg.is_null() {
        let n = NULL_POLLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if n.is_multiple_of(200) {
            record(&format!("idle-polls {} (game thread alive, nothing queued)", n));
        }
        return;
    }
    let m = unsafe { &*msg };
    record(&format!("ovr_PopMessage type={:08X} id={} error={}", m.msg_type, m.id, m.is_error as u32));
}

fn record(name: &str) {
    // Opening the trace must never abort the game: a panic across an FFI
    // boundary is UB. If the log path is unwritable the trace silently
    // stays off; the game keeps running.
    static FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    let file = FILE.get_or_init(|| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
            .ok()
            .map(Mutex::new)
    });
    let Some(f) = file else { return };
    let _ = writeln!(f.lock().unwrap(), "{} {}", now_ms(), name);
}

fn init() {}

static REQUEST_ID: AtomicU32 = AtomicU32::new(1);

/// Entitlement basis the launcher hands over via IRIS_ENTITLEMENT: "verified"
/// means the store confirmed ownership online and it is cached, "fallback"
/// means an imported or never-verified game granted offline so it still boots.
/// Absent env reads as fallback, so a standalone probe boots too. Both grant;
/// the trace records which basis answered.
fn entitlement_basis() -> &'static str {
    match std::env::var("IRIS_ENTITLEMENT").as_deref() {
        Ok("verified") => "verified",
        _ => "fallback",
    }
}

fn entitlement_request() -> u64 {
    let id = REQUEST_ID.fetch_add(1, Ordering::SeqCst);
    record(&format!("entitlement granted: {} ownership", entitlement_basis()));
    let msg = Box::new(Msg {
        msg_type: MSG_ENTITLEMENT,
        is_error: false,
        entitled: true,
        id,
    });
    if let Ok(mut q) = queue().lock() {
        q.push_back(msg);
    }
    id as u64
}

/// Queue a leaderboard result carrying an empty payload and return its request
/// id (ovrRequest is u64). The array/status getters all answer empty (size 0,
/// total 0, didUpdate false), so the leaderboard UI renders empty instead of
/// crashing. Offline, no vendor service; msg_type is the SDK hash so the game
/// recognises the result.
fn leaderboard_request(msg_type: u32) -> u64 {
    let id = REQUEST_ID.fetch_add(1, Ordering::SeqCst);
    let msg = Box::new(Msg { msg_type, is_error: false, entitled: false, id });
    if let Ok(mut q) = queue().lock() {
        q.push_back(msg);
    }
    record(&format!("leaderboard request type={msg_type:08X} id={id} (empty payload)"));
    id as u64
}

fn pop_message() -> *mut Msg {
    if let Ok(mut q) = queue().lock() {
        if let Some(m) = q.pop_front() {
            return Box::into_raw(m);
        }
    }
    std::ptr::null_mut()
}

fn msg_id(msg: *mut Msg) -> u64 {
    if msg.is_null() { 0 } else { unsafe { (*msg).id as u64 } }
}

fn msg_type(msg: *mut Msg) -> u32 {
    if msg.is_null() { 0 } else { unsafe { (*msg).msg_type } }
}

fn msg_is_error(msg: *mut Msg) -> u32 {
    if msg.is_null() { 0 } else { unsafe { (*msg).is_error as u32 } }
}

#[allow(dead_code)]
fn msg_completed(msg: *mut Msg) -> *mut Msg {
    msg
}

#[allow(dead_code)]
fn msg_entitled(msg: *mut Msg) -> u32 {
    if msg.is_null() { 0 } else { unsafe { (*msg).entitled as u32 } }
}

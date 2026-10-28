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
        if n % 200 == 0 {
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

fn entitlement_request() -> u32 {
    let id = REQUEST_ID.fetch_add(1, Ordering::SeqCst);
    let msg = Box::new(Msg {
        msg_type: MSG_ENTITLEMENT,
        is_error: false,
        entitled: true,
        id,
    });
    if let Ok(mut q) = queue().lock() {
        q.push_back(msg);
    }
    id
}

fn pop_message() -> *mut Msg {
    if let Ok(mut q) = queue().lock() {
        if let Some(m) = q.pop_front() {
            return Box::into_raw(m);
        }
    }
    std::ptr::null_mut()
}

fn msg_id(msg: *mut Msg) -> u32 {
    if msg.is_null() { 0 } else { unsafe { (*msg).id } }
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

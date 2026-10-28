include!(concat!(env!("OUT_DIR"), "/generated.rs"));

use std::fs::OpenOptions;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Stable non-NULL handle for every call. One zeroed static record; repeated
/// getters return the same pointer, so retained handles never get recycled.
static RECORD: [u8; 128] = [0; 128];

fn handle() -> *mut u8 {
    &RECORD as *const [u8; 128] as *mut u8
}

fn log_path() -> String {
    std::env::var("OVR_SHIM_LOG").unwrap_or_else(|_| "C:\\oculus-shim-trace.log".to_string())
}

fn record(name: &str) {
    static FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
    let file = FILE.get_or_init(|| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
            .expect("cannot open OVR_SHIM_LOG")
            .into()
    });
    if let Ok(mut f) = file.lock() {
        use std::io::Write;
        let _ = writeln!(f, "{name}");
        let _ = f.flush();
    }
}

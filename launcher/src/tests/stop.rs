use crate::*;

#[test]
fn pid_alive_tracks_a_real_process() {
    let mut child = std::process::Command::new("sleep")
        .arg("5")
        .spawn()
        .unwrap();
    let pid = child.id();
    assert!(pid_alive(pid));
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("kill")
            .arg("-9")
            .arg(pid.to_string())
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .spawn();
    }
    let _ = child.wait();
    assert!(!pid_alive(pid));
}

#[test]
fn running_flag_clears_when_the_process_ends() {
    let slot: Slot<String> = std::sync::Arc::new(std::sync::Mutex::new(None));
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let child = std::process::Command::new("sh")
        .arg("-c")
        .arg("true")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    capture_output(
        child,
        "test game".into(),
        std::path::PathBuf::from("/tmp"),
        slot.clone(),
        flag.clone(),
    );
    for _ in 0..80 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if !flag.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
    }
    assert!(
        !flag.load(std::sync::atomic::Ordering::Relaxed),
        "button must drop back to Play after exit"
    );
}

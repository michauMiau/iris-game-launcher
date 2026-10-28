use crate::*;

#[cfg(unix)]
#[test]
fn capture_output_flags_graphics_failure() {
    // A game that prints Unity's graphics-init refusal must end up with the
    // advice flag in the slot, not silence in the status bar.
    let slot: Slot<String> = std::sync::Arc::new(std::sync::Mutex::new(None));
    let child = std::process::Command::new("sh")
        .arg("-c")
        .arg("echo Failed to initialize graphics; echo InitializeEngineGraphics failed")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    capture_output(
        child,
        "test game".into(),
        std::path::PathBuf::from("/tmp"),
        slot.clone(),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    for _ in 0..160 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if slot.lock().unwrap().is_some() {
            break;
        }
    }
    assert!(slot
        .lock()
        .unwrap()
        .as_ref()
        .map(|r| r.is_ok())
        .unwrap_or(false));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn capture_output_flags_prefix_version_mismatch() {
    // Proton's prefix-version complaint must surface as the "prefix" advice
    // flag, not the graphics one and not silence.
    let slot: Slot<String> = std::sync::Arc::new(std::sync::Mutex::new(None));
    let child = std::process::Command::new("sh")
        .arg("-c")
        .arg("echo \"Proton: Prefix has an invalid version?!\"")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    capture_output(
        child,
        "test game".into(),
        std::path::PathBuf::from("/tmp"),
        slot.clone(),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    for _ in 0..160 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if slot.lock().unwrap().is_some() {
            break;
        }
    }
    assert_eq!(
        slot.lock().unwrap().as_ref().map(|r| r.as_deref()),
        Some(Ok("prefix"))
    );
}

#[test]
fn toggles_touch_only_their_own_entry() {
    let mut env = vec![
        "WINEDLLOVERRIDES=winewayland=d".to_string(),
        "GAME=1".to_string(),
    ];
    assert!(toggle_dll_override(&mut env, "version=n,b", true));
    assert_eq!(env[0], "WINEDLLOVERRIDES=winewayland=d,version=n,b");
    assert_eq!(env[1], "GAME=1");
    assert!(
        !toggle_dll_override(&mut env, "version=n,b", true),
        "double add refused"
    );
    assert!(toggle_dll_override(&mut env, "winewayland=d", false));
    assert_eq!(env[0], "WINEDLLOVERRIDES=version=n,b");
    let mut mid = vec!["WINEDLLOVERRIDES=a=n,version=n,b,c=b".to_string()];
    assert!(toggle_dll_override(&mut mid, "version=n,b", false));
    assert_eq!(
        mid[0], "WINEDLLOVERRIDES=a=n,c=b",
        "neighbour entries survive"
    );
    assert!(has_dll_override(&env, "version=n,b"));
    assert!(toggle_dll_override(&mut env, "version=n,b", false));
    assert_eq!(
        env,
        vec!["GAME=1".to_string()],
        "empty override line removed"
    );
    let mut args = vec!["-force-gfx-directx11".to_string()];
    assert!(toggle_arg(&mut args, "-force-glcore", true));
    assert!(toggle_arg(&mut args, "-force-gfx-directx11", false));
    assert_eq!(args, vec!["-force-glcore".to_string()]);
    assert!(toggle_arg(&mut args, "-force-glcore", false));
    assert!(args.is_empty());
    let mut env2 = vec!["WINEDLLOVERRIDES=dxgi=n".to_string()];
    assert!(toggle_env_entry(
        &mut env2,
        "OVR_SHIM_LOG",
        "/games/x/oculus-shim-trace.log",
        true
    ));
    assert_eq!(env2.len(), 2);
    assert!(has_env_key(&env2, "OVR_SHIM_LOG"));
    assert!(toggle_env_entry(
        &mut env2,
        "OVR_SHIM_LOG",
        "/games/x/oculus-shim-trace.log",
        false
    ));
    assert_eq!(env2, vec!["WINEDLLOVERRIDES=dxgi=n".to_string()]);
}

#[cfg(not(target_os = "windows"))]
#[test]
fn protonfixes_always_gets_a_numeric_game_id() {
    // GE-Proton's protonfixes calls exit() when no id is found, which would
    // abort the game launch outright.
    assert_eq!(
        steam_app_id_for("Bonelab", "5088709007839657"),
        "5088709007839657"
    );
    let synth = steam_app_id_for("stress-level-zero-inc-boneworks", "");
    assert_eq!(
        synth.len(),
        10,
        "synthetic ids stay outside the real Steam id space"
    );
    assert!(synth.chars().all(|c| c.is_ascii_digit()));
    assert_ne!(synth, steam_app_id_for("Bonelab rip copy", ""));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn prefix_env_sets_the_steam_compat_vars() {
    let mut cmd = std::process::Command::new("wine");
    prefix_env(&mut cmd, std::path::Path::new("/tmp/pfx"));
    let z = cmd
        .get_envs()
        .find(|(k, _)| k.to_string_lossy() == "WINEPREFIX")
        .and_then(|(_, v)| v.map(|v| v.to_string_lossy().to_string()));
    assert_eq!(z.as_deref(), Some("/tmp/pfx"));
    assert!(cmd
        .get_envs()
        .any(|(k, _)| k.to_string_lossy() == "STEAM_COMPAT_DATA_PATH"));
}

#[test]
fn melon_dirs_creates_the_loader_folders() {
    let dir = std::env::temp_dir().join(format!("iris-melon-dirs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(melon_dirs(&dir), 4);
    for name in ["Mods", "Plugins", "UserData", "UserLibs"] {
        assert!(dir.join(name).is_dir(), "{name} missing");
    }
    assert_eq!(melon_dirs(&dir), 4);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn update_check_says_available_or_none() {
    let mut g = GameEntry {
        description: String::new(),
        app_id: "1".into(),
        name: "Test".into(),
        install_dir: "/games/test".into(),
        launch_exe: "test.exe".into(),
        version: "1.0".into(),
        meta_app_id: "1".into(),
        binary_id: String::new(),
        cover_url: String::new(),
        entitled: true,
        last_played: 0,
        partial: false,
        version_code: 100,
        branches: vec![fetcher::Branch {
            name: "public".into(),
            binary_id: "9".into(),
            version: "1.1".into(),
            version_code: 200,
        }],
        proton: String::new(),
        launch_args: Vec::new(),
        launch_env: Vec::new(),
    };
    assert!(update_verdict(&g, 0).starts_with("update available"));
    g.branches[0].version_code = 100;
    assert_eq!(
        update_verdict(&g, 0),
        "no update available — this build is the newest"
    );
    g.install_dir = String::new();
    assert_eq!(
        update_verdict(&g, 0),
        "not installed yet — nothing to update"
    );
    g.install_dir = "/games/test".into();
    g.branches.clear();
    assert!(update_verdict(&g, 0).starts_with("no store channels"));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn data_folder_finds_the_proton_user_dir() {
    let root = std::env::temp_dir().join(format!("iris-locallow-{}", std::process::id()));
    let user = root.join("drive_c/users/steamuser");
    std::fs::create_dir_all(user.join("AppData/LocalLow/SLZ/Marrow")).unwrap();
    std::fs::write(
        user.join("AppData/LocalLow/SLZ/Marrow/Player.log"),
        "loading\n",
    )
    .unwrap();
    let low = data_folder(Some(&root)).unwrap();
    assert_eq!(low, user.join("AppData").join("LocalLow"));
    let pfx_root = std::env::temp_dir().join(format!("iris-locallow-pfx-{}", std::process::id()));
    let puser = pfx_root.join("pfx/drive_c/users/steamuser");
    std::fs::create_dir_all(puser.join("AppData/LocalLow/SLZ/Marrow")).unwrap();
    assert_eq!(
        data_folder(Some(&pfx_root)).unwrap(),
        puser.join("AppData").join("LocalLow"),
        "proton pfx layout must be found too"
    );
    std::fs::remove_dir_all(pfx_root).ok();
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn import_folder_finds_a_rip_and_refuses_a_twin() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let root = std::env::temp_dir().join(format!("iris-imp-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::env::set_var("XDG_DATA_HOME", &root);
    std::fs::create_dir_all(root.join("rip/stress-level-zero-inc-boneworks")).unwrap();
    std::fs::write(
        root.join("rip/stress-level-zero-inc-boneworks/Boneworks_Oculus_Windows64.exe"),
        b"exe",
    )
    .unwrap();
    let mut app = App::new(false);
    // A folder with no store identity now asks which library game it is.
    app.import_folder(root.join("rip"));
    assert!(app.import_ask.is_some(), "unmatched folder should ask");
    assert_eq!(app.store.apps.len(), 0, "ask must not push yet");
    app.plain_import(&root.join("rip/stress-level-zero-inc-boneworks"));
    assert_eq!(app.store.apps.len(), 1, "plain import after the ask works");
    assert_eq!(app.store.apps[0].name, "stress-level-zero-inc-boneworks");
    assert_eq!(
        app.store.apps[0].launch_exe,
        "Boneworks_Oculus_Windows64.exe"
    );
    app.import_folder(root.join("rip"));
    assert_eq!(app.store.apps.len(), 1, "same folder must not twin");
    assert!(app.status.contains("already"));
    app.import_folder(root.join("rip/nowhere"));
    assert!(app.status.contains("no Windows exe"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn import_matches_a_folder_by_oculus_manifest_app_id() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let root = std::env::temp_dir().join(format!("iris-imp-match-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::env::set_var("XDG_DATA_HOME", &root);
    let game = root.join("copy").join("bonelab");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(game.join("BONELAB_Oculus_Windows64.exe"), b"exe").unwrap();
    std::fs::write(
        game.join(".oculus-manifest.json"),
        br#"{
            "appId": "5088709007839657",
            "canonicalName": "Bonelab VR",
            "launchFile": "Bonelab/BONELAB_Oculus_Windows64.exe",
            "version": "1.1403.57485"
        }"#,
    )
    .unwrap();
    let mut app = App::new(false);
    app.store.apps.push(GameEntry {
        description: String::new(),
        app_id: "5088709007839657".into(),
        name: "Bonelab VR".into(),
        install_dir: String::new(),
        launch_exe: String::new(),
        version: "old".into(),
        meta_app_id: "5088709007839657".into(),
        binary_id: String::new(),
        cover_url: String::new(),
        entitled: true,
        last_played: 0,
        version_code: 0,
        launch_args: Vec::new(),
        launch_env: Vec::new(),
        partial: false,
        branches: Vec::new(),
        proton: String::new(),
    });
    app.import_folder(root.join("copy"));
    assert_eq!(app.store.apps.len(), 1, "match must add no second entry");
    let g = &app.store.apps[0];
    assert_eq!(
        g.install_dir,
        game.display().to_string(),
        "folder bound to entry"
    );
    assert_eq!(g.launch_exe, "BONELAB_Oculus_Windows64.exe");
    assert_eq!(
        g.version, "1.1403.57485",
        "manifest version fills the entry"
    );
    assert_eq!(g.meta_app_id, "5088709007839657", "store id kept");
    assert!(app.status.contains("matched"), "status says matched");
    std::fs::remove_dir_all(root).ok();
}

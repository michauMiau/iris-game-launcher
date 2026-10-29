use crate::*;

#[test]
#[cfg(target_os = "windows")]
fn revive_scan_finds_the_layer_and_runtime_peer() {
    let root = std::env::temp_dir().join(format!("iris-revscan-{}", std::process::id()));
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::create_dir_all(root.join("c")).unwrap();
    std::fs::write(root.join("a/b/LibOVRRT64_1.dll"), b"lib").unwrap();
    std::fs::write(root.join("c/OVRServer_x64.exe"), b"srv").unwrap();
    let (dll, server) = revive_scan(std::slice::from_ref(&root));
    assert_eq!(dll.unwrap(), root.join("a/b/LibOVRRT64_1.dll"));
    assert_eq!(server.unwrap(), root.join("c/OVRServer_x64.exe"));
    let (none_dll, none_srv) = revive_scan(&[root.join("nope")]);
    assert!(none_dll.is_none() && none_srv.is_none());
    std::fs::remove_dir_all(root).ok();
}

#[cfg(target_os = "windows")]
#[test]
fn revive_dll_step_copies_beside_the_game_exe() {
    let root = std::env::temp_dir().join(format!("iris-revive-{}", std::process::id()));
    std::fs::create_dir_all(root.join("runtime")).unwrap();
    std::fs::create_dir_all(root.join("game")).unwrap();
    std::fs::write(root.join("runtime/LibOVRRT64_1.dll"), b"lib").unwrap();
    let target = revive_dll_step(
        root.join("runtime/LibOVRRT64_1.dll").to_str().unwrap(),
        &root.join("game"),
    )
    .expect("valid dll copies into the game folder");
    assert_eq!(target, root.join("game/LibOVRRT64_1.dll"));
    assert!(target.is_file());
    assert!(revive_dll_step("", &root.join("game")).is_none());
    assert!(revive_dll_step(
        root.join("runtime/LibOVRRT64_1.dll").to_str().unwrap(),
        &root.join("nope"),
    )
    .is_none());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn steam_install_path_parsed_from_reg_output() {
    let out = "\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Valve\\Steam\n    InstallPath    REG_SZ    C:\\Program Files (x86)\\Steam\n";
    assert_eq!(
        steam_install_path_from_reg(out).as_deref(),
        Some(r"C:\Program Files (x86)\Steam")
    );
    assert_eq!(steam_install_path_from_reg("no value here"), None);
}

#[test]
fn tasklist_image_match_and_miss() {
    assert!(tasklist_has_image(
        "Image Name    vrserver.exe    Running",
        "vrserver.exe"
    ));
    assert!(!tasklist_has_image(
        "INFO: No tasks are running which match the specified criteria.",
        "vrserver.exe"
    ));
}

#[test]
fn xr_manifest_finds_json_declaring_library_path() {
    let dir = std::env::temp_dir().join(format!("iris-xrman-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("other.json"), b"{\"nope\":1}").unwrap();
    std::fs::write(
        dir.join("steamxr_win.json"),
        b"{\"runtime\":{\"library_path\":\"bin/vrclient.dll\"}}",
    )
    .unwrap();
    assert_eq!(xr_manifest(&dir), Some(dir.join("steamxr_win.json")));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn dll_matches_checks_byte_length() {
    let f = std::env::temp_dir().join(format!("iris-dllmatch-{}", std::process::id()));
    std::fs::write(&f, b"abc").unwrap();
    assert!(dll_matches(&f, 3));
    assert!(!dll_matches(&f, 4));
    assert!(!dll_matches(&f.join("missing"), 3));
    std::fs::remove_file(f).ok();
}

#[cfg(not(target_os = "windows"))]
#[test]
fn steamxr_manifest_found_only_next_to_the_runtime_root() {
    let root = std::env::temp_dir().join(format!("iris-xrjson-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(steamxr_manifest(&root), None);
    std::fs::write(root.join("steamxr_linux64.json"), b"{\"runtime\":{}}").unwrap();
    assert_eq!(
        steamxr_manifest(&root),
        Some(root.join("steamxr_linux64.json"))
    );
    std::fs::remove_dir_all(root).ok();
}

#[cfg(not(target_os = "windows"))]
#[test]
fn steamvr_runtime_root_requires_the_native_vrclient() {
    let root = std::env::temp_dir().join(format!("iris-steamvr-{}", std::process::id()));
    let home = root.join("home");
    // A SteamVR dir without bin/linux64/vrclient.so is not a runtime root;
    // Proton appends that path itself and fails if it is missing.
    std::fs::create_dir_all(home.join(".steam/steam/steamapps/common/SteamVR")).unwrap();
    assert_eq!(steamvr_runtime_root(home.to_str().unwrap()), None);
    std::fs::create_dir_all(home.join(".local/share/Steam/steamapps/common/SteamVR/bin/linux64"))
        .unwrap();
    std::fs::write(
        home.join(".local/share/Steam/steamapps/common/SteamVR/bin/linux64/vrclient.so"),
        b"stub",
    )
    .unwrap();
    assert_eq!(
        steamvr_runtime_root(home.to_str().unwrap()),
        Some(home.join(".local/share/Steam/steamapps/common/SteamVR"))
    );
    std::fs::remove_dir_all(root).ok();
}

#[cfg(not(target_os = "windows"))]
#[test]
fn proton_vr_runtime_entry_toggles_only_itself() {
    let mut env = vec![
        "WINEDLLOVERRIDES=winewayland=d".to_string(),
        "PROTON_VR_RUNTIME=/usr/share/SteamVR".to_string(),
    ];
    assert!(has_env_key(&env, "PROTON_VR_RUNTIME"));
    assert!(toggle_env_entry(&mut env, "PROTON_VR_RUNTIME", "/x", false));
    assert_eq!(env, vec!["WINEDLLOVERRIDES=winewayland=d".to_string()]);
    assert!(toggle_env_entry(
        &mut env,
        "PROTON_VR_RUNTIME",
        "/usr/share/SteamVR",
        true
    ));
    assert_eq!(env.len(), 2);
    assert!(!toggle_env_entry(
        &mut env,
        "PROTON_VR_RUNTIME",
        "/other",
        true
    ));
    assert_eq!(
        env[1], "PROTON_VR_RUNTIME=/usr/share/SteamVR",
        "existing runtime path is kept"
    );
}

#[test]
fn proton_vr_capability_reads_ge_build_names() {
    let p = |n: &str| std::path::PathBuf::from(format!("/tools/{n}/proton"));
    assert!(proton_vr_capable(&p("GE-Proton10-33")));
    assert!(proton_vr_capable(&p("GE-Proton11-1")));
    assert!(!proton_vr_capable(&p("GE-Proton10-29")));
    assert!(!proton_vr_capable(&p("Proton - Experimental")));
    assert!(!proton_vr_capable(&p("cachyos-11.0-20261015-slr")));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn abstract_pipe_names_found_and_dir_derived() {
    let sample = "ffff000000000000 2 0 0 00000000 12345 @/tmp/steamvr/vrserver_pipe_99_0\n\
ffff000000000001 2 0 0 00000000 678 /home/u/.config/SteamVR/vrserver_pipe_55_1\n\
ffff000000000002 1 100 0 00000000 999 /tmp/other_socket\n";
    let pipes = crate::vr_abstract_pipes(sample);
    assert_eq!(pipes.len(), 2, "only vrserver pipes are picked");
    assert!(pipes.contains(&"/tmp/steamvr/vrserver_pipe_99_0".to_string()));
    assert!(pipes.contains(&"/home/u/.config/SteamVR/vrserver_pipe_55_1".to_string()));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn pipe_relay_symlinks_reach_the_server_side_dir() {
    // The relay reads the server dir through /proc/<pid>/root. Using our own
    // pid keeps the test real: /proc/self/root resolves to this filesystem.
    let _guard = crate::ENV_LOCK.lock().expect("env lock");
    let tag = format!("iris-relay-{}", std::process::id());
    let root = std::env::temp_dir().join(&tag);
    std::fs::remove_dir_all(&root).ok();
    std::env::set_var("XDG_DATA_HOME", &root);
    let pipes = root.join("steamvr");
    std::fs::create_dir_all(&pipes).unwrap();
    std::fs::write(pipes.join("vrserver_pipe_4242_0"), b"socket").unwrap();
    std::fs::write(pipes.join("not-a-pipe.txt"), b"x").unwrap();
    let rel = pipes.strip_prefix("/").unwrap();
    let relay = crate::vr_pipe_relay(
        &std::process::id().to_string(),
        &std::path::Path::new("/").join(rel),
    )
    .expect("relay must find the pipe file");
    let linked = relay.join("vrserver_pipe_4242_0");
    assert!(linked.exists(), "relay must symlink the vrserver pipe");
    assert!(
        !relay.join("not-a-pipe.txt").exists(),
        "non-pipe files stay out"
    );
    assert!(std::fs::read_link(&linked)
        .unwrap()
        .to_string_lossy()
        .contains(&format!("/proc/{}/root", std::process::id())));
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&relay).ok();
}

#[cfg(not(target_os = "windows"))]
#[test]
fn wine64_found_beside_proton_layout() {
    let _guard = crate::ENV_LOCK.lock().expect("env lock");
    let tag = format!("iris-wine64-{}", std::process::id());
    let root = std::env::temp_dir().join(&tag);
    std::fs::remove_dir_all(&root).ok();
    let proton = root.join("GE-Proton10-33");
    std::fs::create_dir_all(proton.join("files/bin")).unwrap();
    std::fs::write(proton.join("proton"), b"#!/bin/sh\n").unwrap();
    std::fs::write(proton.join("files/bin/wine64"), b"x").unwrap();
    let found = crate::wine64_for(&proton.join("proton"));
    assert_eq!(found, Some(proton.join("files/bin/wine64")));
    std::fs::remove_dir_all(&root).ok();
}

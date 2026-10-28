use crate::*;

#[test]
fn login_link_rejections_render_as_errors_not_success() {
    assert_eq!(
        status_tone("the login link is missing its login token"),
        "error"
    );
    assert_eq!(status_tone("that link is not for this login"), "error");
    assert_eq!(
        status_tone("the store did not accept this login token - log in again"),
        "error"
    );
}

#[test]
fn removed_game_notice_is_not_green() {
    assert_eq!(
        status_tone("this game is no longer in the library"),
        "guidance"
    );
}

#[test]
fn long_status_text_is_clipped_to_the_space_left() {
    assert_eq!(fit_text("ready", 1000.0), "ready");
    let clipped = fit_text(&"x".repeat(200), 40.0);
    assert!(clipped.chars().count() <= 6, "clipped to fit: {clipped}");
    assert!(clipped.ends_with('…'));
    assert_eq!(fit_text(&"x".repeat(200), 0.0), "…");
}

#[test]
fn long_paths_get_abbreviated() {
    assert_eq!(
        short_path("/home/user/.local/steam/steamapps/common"),
        "/h.../steamapps/common"
    );
    // Short paths stay verbatim — abbreviating them would hide info.
    assert_eq!(short_path("/usr/bin"), "/usr/bin");
    assert_eq!(short_path("/opt/steam/common"), "/opt/steam/common");
    assert_eq!(
        short_path("C:\\Steam\\steamapps\\common"),
        "C:\\Steam\\steamapps\\common"
    );
}

#[test]
fn rescan_registers_a_store_owned_folder_instead_of_a_twin_card() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    // A cancelled store install leaves install_dir empty while the folder
    // stays on disk. Rescan must write that folder onto the store entry, not
    // push a second local card claiming "installed".
    let tmp = std::env::temp_dir().join(format!("iris-rescan-{}", std::process::id()));
    let dl = tmp.join("games");
    let folder = dl.join("bonelab");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("game.exe"), b"MZ").unwrap();

    let mut app = App::new(false);
    app.store_path = tmp.join("store.json");
    app.config.download_dir = dl.to_string_lossy().to_string();
    app.config.prefix_dir = String::new();
    let saved_home = std::env::var("HOME").unwrap_or_default();
    // Hide the host's real Steam library from the scan so the test sees only
    // its own folder.
    std::env::set_var("HOME", tmp.join("nohome"));
    // App::new loaded the real library; the test runs against a private store.
    app.store.apps.clear();
    app.store.apps.push(GameEntry {
        description: String::new(),
        app_id: "42".to_string(),
        name: "Bonelab".to_string(),
        install_dir: String::new(),
        launch_exe: String::new(),
        version: String::new(),
        meta_app_id: "42".to_string(),
        binary_id: String::new(),
        cover_url: String::new(),
        entitled: true,
        last_played: 0,
        partial: true,
        version_code: 0,
        branches: Vec::new(),
        proton: String::new(),
        launch_args: Vec::new(),
        launch_env: Vec::new(),
    });
    app.rescan_dirs();

    assert_eq!(app.store.apps.len(), 1, "no twin card");
    let e = &app.store.apps[0];
    assert!(
        !e.install_dir.is_empty(),
        "folder registered on the store entry"
    );
    assert!(!e.launch_exe.is_empty());
    assert!(!e.partial, "a folder with an exe is not partial");
    std::fs::remove_dir_all(&tmp).ok();
    std::env::set_var("HOME", saved_home);
}

#[test]
fn survives_missing_and_malformed_data_files() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = std::env::temp_dir().join(format!("iris-resil-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.json"), b"{not json at all").unwrap();
    std::fs::write(dir.join("games.json"), b"[truncated").unwrap();
    std::fs::write(dir.join("session.json"), b"}").unwrap();
    std::env::set_var("XDG_DATA_HOME", &dir);
    let app = App::new(false);
    assert_eq!(app.store.apps.len(), 0);
    assert!(app.session.token.is_empty());
    assert!(app.config.prefix_dir.contains("iris-launcher"));
    std::env::remove_var("XDG_DATA_HOME");
    std::fs::remove_dir_all(&dir).ok();
    std::env::set_var("XDG_DATA_HOME", dir.join("gone"));
    let app2 = App::new(false);
    assert_eq!(app2.store.apps.len(), 0);
    assert!(app2.session.token.is_empty());
    std::env::remove_var("XDG_DATA_HOME");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn pasted_token_survives_untouched_file_but_external_write_reloads() {
    let dir = std::env::temp_dir().join("iris-session-refresh-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("session.json");

    // Disk holds the old placeholder; GUI memory holds a pasted bearer.
    std::fs::write(
        &path,
        serde_json::to_string(&Session {
            token: "old-placeholder".into(),
            ..Session::default()
        })
        .unwrap(),
    )
    .unwrap();
    let mut remembered = std::fs::read(&path).unwrap();
    let mut session = Session {
        token: "pasted-bearer".into(),
        ..Session::default()
    };

    // File bytes unchanged since last read: the paste must survive.
    assert!(!apply_disk_session(&mut session, &mut remembered, &path));
    assert_eq!(session.token, "pasted-bearer");

    // A handler process writes a fresh callback: reload it.
    std::fs::write(
        &path,
        serde_json::to_string(&Session {
            token: "callback-bearer".into(),
            frl_request: "tok".into(),
            frl_blob: "blob".into(),
            ..Session::default()
        })
        .unwrap(),
    )
    .unwrap();
    assert!(apply_disk_session(&mut session, &mut remembered, &path));
    assert_eq!(session.token, "callback-bearer");
    assert_eq!(session.frl_request, "tok");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn newer_size_answer_wins_the_slot() {
    // Old worker must not overwrite the popup's newer lookup.
    assert!(size_write_allowed(None, 1));
    assert!(size_write_allowed(Some(1), 2));
    assert!(size_write_allowed(Some(2), 2));
    assert!(!size_write_allowed(Some(2), 1));
}

#[test]
fn guidance_never_reads_as_green_success() {
    // Measured bug: "still measuring" and "nothing installed to remove" fell
    // through both lists and rendered in the green success color.
    assert_eq!(status_tone("still measuring the bundle size"), "guidance");
    assert_eq!(status_tone("nothing installed to remove"), "guidance");
    assert_eq!(status_tone("not signed in yet — log in first"), "guidance");
    assert_eq!(status_tone("this game is not installed yet"), "guidance");
    assert_eq!(
        status_tone("no Steam here — installing into the launcher folder instead"),
        "guidance"
    );
    assert_eq!(
        status_tone("files check: 10 ok, 1 corrupt, 2 missing — install again to repair"),
        "error"
    );
    assert_eq!(status_tone("download failed: sha256 mismatch"), "error");
    assert_eq!(status_tone("bonelab installed"), "success");
}

#[test]
fn melon_dest_targets_the_popup_game_not_another_entry() {
    let apps: Vec<GameEntry> = serde_json::from_str(
        r#"[
          {"app_id":"a","name":"Bonelab","install_dir":"/games/bonelab","launch_exe":"x.exe","version":"1"},
          {"app_id":"b","name":"Other","install_dir":"/games/other","launch_exe":"y.exe","version":"1"},
          {"app_id":"c","name":"Fresh","install_dir":"","launch_exe":"","version":"1"}
        ]"#,
    )
    .unwrap();
    assert_eq!(melon_dest(&apps, 0), Some(PathBuf::from("/games/bonelab")));
    assert_eq!(melon_dest(&apps, 1), Some(PathBuf::from("/games/other")));
    assert_eq!(melon_dest(&apps, 2), None);
    assert_eq!(melon_dest(&apps, 9), None);
}

#[test]
fn install_folder_names_the_game_like_the_official_launcher() {
    // Popup default must be games/<name>, not the games root bare.
    assert_eq!(folder_name("Bonelab VR"), "bonelab-vr");
    assert_eq!(folder_name("BONELAB"), "bonelab");
    assert_eq!(folder_name("  weird!! name??  "), "weird-name");
}

#[test]
fn marker_never_reads_signed_in_without_a_token() {
    // verified must die with the token (Clear session path).
    assert_eq!(marker(false, true, false), ("not signed in", "off"));
    assert_eq!(marker(true, true, false), ("signed in", "ok"));
    assert_eq!(marker(true, false, false), ("token not verified", "warn"));
    assert_eq!(marker(true, false, true), ("checking account", "wait"));
    assert_eq!(marker(false, false, false), ("not signed in", "off"));
}

#[test]
fn relative_launch_exe_is_joined_back_onto_the_install_dir() {
    // Store downloads store a name relative to the install folder.
    assert_eq!(
        resolve_launch_path("/data/games/bonelab", "Bonelab.exe"),
        PathBuf::from("/data/games/bonelab/Bonelab.exe")
    );
    // Rescan entries are already absolute and must stay untouched.
    assert_eq!(
        resolve_launch_path("/data/games/bonelab", "/opt/Bonelab.exe"),
        PathBuf::from("/opt/Bonelab.exe")
    );
}

#[test]
fn locale_and_desktop_hints_map_to_shipped_ui_values() {
    assert_eq!(lang_from_locale("pl_PL.UTF-8"), Some("pl"));
    assert_eq!(lang_from_locale("en_US:en"), Some("en"));
    assert_eq!(lang_from_locale("C.UTF-8"), None);
    assert_eq!(
        lang_from_reg_locale("Locale    REG_SZ    0x0415"),
        Some("pl")
    );
    assert_eq!(
        lang_from_reg_locale("Locale    REG_SZ    en-US:0x409"),
        Some("en")
    );
    assert_eq!(lang_from_reg_locale("no locale line here"), None);
    assert_eq!(
        theme_from_reg_light("AppsUseLightTheme    REG_DWORD    0x0"),
        Some("dark")
    );
    assert_eq!(
        theme_from_reg_light("AppsUseLightTheme    REG_DWORD    0x1"),
        Some("light")
    );
    assert_eq!(theme_from_reg_light("junk"), None);
    assert_eq!(theme_from_gnome_scheme("'prefer-dark'"), Some("dark"));
    assert_eq!(theme_from_gnome_scheme("'prefer-light'"), Some("light"));
    assert_eq!(theme_from_gnome_scheme("'no-preference'"), None);
    assert_eq!(
        theme_from_kde_globals("[General]\nColorScheme=Breeze\n"),
        None
    );
    assert_eq!(
        theme_from_kde_globals("[General]\nColorScheme=Breeze Dark\n"),
        Some("dark")
    );
    assert_eq!(
        theme_from_kde_globals("[General]\nColorScheme=Light\n"),
        Some("light")
    );
}

#[test]
fn index_keyed_popups_reset_when_library_shrinks() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let mut app = App::new(false);
    app.store.apps.push(GameEntry {
        description: String::new(),
        app_id: "a".into(),
        name: "Alpha".into(),
        install_dir: String::new(),
        launch_exe: String::new(),
        version: String::new(),
        meta_app_id: "a".into(),
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
    app.store.apps.push(GameEntry {
        description: String::new(),
        app_id: "b".into(),
        name: "Beta".into(),
        install_dir: String::new(),
        launch_exe: String::new(),
        version: String::new(),
        meta_app_id: "b".into(),
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
    app.tweak_popup = Some(1);
    app.custom_pick = Some(1);
    #[cfg(not(target_os = "windows"))]
    {
        app.game_proton_popup = Some(1);
    }
    app.fetch_inflight = true;
    *app.fetch_result.lock().unwrap() = Some((
        app.session.token.clone(),
        Ok(vec![fetcher::AppInfo {
            id: "a".into(),
            display_name: "Alpha".into(),
            description: String::new(),
            entitled: true,
            binary_id: String::new(),
            version: String::new(),
            version_code: 0,
            cover_url: String::new(),
        }]),
    ));
    app.poll_fetch();
    assert_eq!(app.store.apps.len(), 1, "Beta dropped");
    assert!(
        app.tweak_popup.is_none(),
        "tweak popup must not point at a shifted index"
    );
    assert!(app.custom_pick.is_none(), "custom pick must reset too");
    #[cfg(not(target_os = "windows"))]
    {
        assert!(app.game_proton_popup.is_none());
    }
}

//! Command-line interface: `--help`, the headless commands and the window-run
//! flags. Kept out of `main.rs` so the whole argument surface has one home
//! (documented in `docs/cli.md`).

use crate::{auth, App};

fn print_help() {
    println!("Iris Game Launcher {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage: iris-launcher [option]");
    println!();
    println!("  --help                    print this list");
    println!("  --launch <game name>      launch a game by name");
    println!("  rescan                    re-scan the library folders");
    println!("  --download <id> [folder]  download a store game by id");
    println!("  --reset-prefix            delete all per-game Wine prefixes");
    println!("  --install-handler         take over the login link");
    println!("  --uninstall-handler       give the login link back");
    println!("  login-url                 print the browser login URL");
    println!("  --url <callback url>      apply a login callback link");
    println!("  --har <file>              apply a login callback from a HAR export");
    println!("  --settings-page           open on the settings page");
    println!("  --safe                    software rendering + X11 (or IRIS_SAFE=1)");
    println!("  --no-covers               grey cards instead of cover art (or IRIS_NO_COVERS=1)");
    println!("  --x11                     force the X11 backend");
    println!();
    println!("See docs/cli.md for details.");
}

/// Handle a headless invocation. Returns true when the process should exit
/// without opening the window.
pub fn dispatch(args: &[String]) -> bool {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return true;
    }
    // Hidden --launch <name>: used by per-game .desktop entries created at
    // launch (see write_game_desktop_entry) so the game gets its own
    // taskbar/dock identity instead of the launcher's.
    if args.get(1).map(|a| a == "--launch").unwrap_or(false) {
        let Some(name) = args.get(2) else {
            println!("usage: --launch <game name>");
            std::process::exit(64);
        };
        // This is how per-game .desktop entries launch a game directly from
        // the app grid: set the selected game, then let the normal launch()
        // path run it (SteamVR env, prefix, shim placement included).
        let mut app = App::new(false);
        app.rescan_dirs();
        let idx = app
            .store
            .apps
            .iter()
            .position(|g| &g.name == name || g.name.eq_ignore_ascii_case(name));
        let Some(idx) = idx else {
            eprintln!("unknown game: {name}");
            std::process::exit(2);
        };
        app.selected = idx;
        app.selected_branch = 0;
        auth::log_line(&format!("headless --launch: {name}"));
        app.launch();
        println!("launched");
        return true;
    }
    if args.get(1).map(|a| a == "rescan").unwrap_or(false) {
        let mut app = App::new(false);
        app.rescan_dirs();
        println!("{}", app.status);
        return true;
    }
    if args.get(1).map(|a| a == "--download").unwrap_or(false) {
        let Some(id) = args.get(2) else {
            println!("usage: --download <meta app id>");
            return true;
        };
        let mut app = App::new(false);
        let dest_override = args.get(3).map(|a| a.as_str());
        let msg = app.download_cli(id, dest_override);
        println!("{msg}");
        if msg.contains("failed") || msg.contains("no exe") {
            std::process::exit(1);
        }
        return true;
    }
    if args.get(1).map(|a| a == "--reset-prefix").unwrap_or(false) {
        // Delete the per-game prefixes under the configured prefix root. The
        // library store is untouched; games just need Wine re-initialising.
        let app = App::new(false);
        let root = crate::game_prefix_root(&app.config.prefix_dir);
        if root == std::path::Path::new("/") || root == std::path::Path::new("") {
            println!("refusing to wipe the filesystem root as a prefix dir");
            std::process::exit(1);
        }
        let mut removed = 0usize;
        if let Ok(rd) = std::fs::read_dir(&root) {
            for sub in rd.flatten() {
                if sub.path().is_dir() && std::fs::remove_dir_all(sub.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        println!("{removed} game prefixes removed under {}", root.display());
        return true;
    }
    if args.get(1).map(|a| a == "install-handler").unwrap_or(false) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        match auth::install_oculus_handler("en") {
            Ok(p) => println!("headset link handler installed: {p}"),
            Err(e) => eprintln!("handler install failed: {e}"),
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        eprintln!("no headset link handler on this platform");
        return true;
    }
    if args
        .get(1)
        .map(|a| a == "uninstall-handler")
        .unwrap_or(false)
    {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        match auth::uninstall_oculus_handler("en") {
            Ok(p) => println!("{p}"),
            Err(e) => eprintln!("handler uninstall failed: {e}"),
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        eprintln!("no headset link handler on this platform");
        return true;
    }
    if args.get(1).map(|a| a == "login-url").unwrap_or(false) {
        match auth::start_login() {
            Ok((url, _, _)) => println!("{url}"),
            Err(e) => eprintln!("login start failed: {e}"),
        }
        return true;
    }
    if args.get(1).map(|a| a == "--url").unwrap_or(false) {
        let Some(url) = args.get(2) else {
            eprintln!("missing callback url");
            return true;
        };
        let mut app = App::new(false);
        match auth::callback_params(url) {
            Some((token_param, blob)) => {
                app.session.frl_request = token_param.clone();
                app.session.frl_blob = blob.clone();
                auth::log_line(&format!(
                    "callback via --url token={} blob={}",
                    auth::log_peek(&token_param),
                    auth::log_peek(&blob)
                ));
                // request_token must be the native SSO token whose sha256[..16]
                // equals the URL token param, same gate as the GUI redeem.
                let mut request_token: Option<String> = None;
                if !app.session.native_sso_token.is_empty()
                    && auth::callback_token_matches(&app.session.native_sso_token, &token_param)
                {
                    request_token = Some(app.session.native_sso_token.clone());
                }
                if request_token.is_none()
                    && !app.session.native_sso_manual.is_empty()
                    && auth::callback_token_matches(&app.session.native_sso_manual, &token_param)
                {
                    request_token = Some(app.session.native_sso_manual.clone());
                }
                let Some(rt) = request_token else {
                    app.save_session();
                    println!(
                        "login link stored, but its login token doesn't match the login started in this app — press Login here first, or use --har"
                    );
                    std::process::exit(1);
                };
                match auth::exchange(&rt, &app.session.native_sso_etoken.clone(), &blob) {
                    Ok(bearer) => {
                        app.session.token = bearer;
                        app.session.frl_request.clear();
                        app.session.frl_blob.clear();
                        app.save_session();
                        println!("login token saved");
                    }
                    Err(e) => {
                        app.session.callback_attempted = true;
                        app.save_session();
                        println!("callback stored, login token failed: {e}");
                        println!("retry with: iris-launcher --url \"{url}\" or the Apply button in settings");
                        std::process::exit(1);
                    }
                }
            }
            None => {
                eprintln!("the login link needs a login token and a login code");
                std::process::exit(1);
            }
        }
        return true;
    }
    if args.get(1).map(|a| a == "--har").unwrap_or(false) {
        // Chrome/Firefox drop protocol redirects silently and the vendor wipes the
        // console, so the callback only survives in a Preserve-log HAR export.
        // The approved endpoint returns the redirect inside its JSON response.
        let Some(path) = args.get(2) else {
            eprintln!("missing har file path");
            return true;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("cannot read {path}");
            return true;
        };
        let Ok(har) = serde_json::from_str::<serde_json::Value>(&text) else {
            eprintln!("{path} is not valid HAR json");
            return true;
        };
        let mut found: Option<String> = None;
        if let Some(entries) = har
            .get("log")
            .and_then(|l| l.get("entries"))
            .and_then(|e| e.as_array())
        {
            for e in entries {
                let url = e
                    .get("request")
                    .and_then(|r| r.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("");
                if !url.contains("native_sso/approved") {
                    continue;
                }
                let body = e
                    .get("response")
                    .and_then(|r| r.get("content"))
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                // Comet wraps responses as `for (;;);{...}` — strip the guard.
                let json = body.strip_prefix("for (;;);").unwrap_or(body);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                    if let Some(uri) = v
                        .get("payload")
                        .and_then(|p| p.get("uri"))
                        .and_then(|u| u.as_str())
                    {
                        if uri.contains("frl_login") {
                            found = Some(uri.to_string());
                        }
                    }
                }
            }
        }
        match found {
            Some(uri) => {
                let mut app = App::new(false);
                let Some((token_param, blob)) = auth::callback_params(&uri) else {
                    println!(
                        "the login link in that file has no login token or login code to apply"
                    );
                    return true;
                };
                println!("callback found in HAR: login token and login code extracted");
                {
                    auth::log_line(&format!(
                        "callback via --har token={} blob={}",
                        auth::log_peek(&token_param),
                        auth::log_peek(&blob)
                    ));
                    app.session.frl_request = token_param.clone();
                    app.session.frl_blob = blob.clone();
                    let mut request_token: Option<String> = None;
                    if !app.session.native_sso_token.is_empty()
                        && auth::callback_token_matches(&app.session.native_sso_token, &token_param)
                    {
                        request_token = Some(app.session.native_sso_token.clone());
                    }
                    if request_token.is_none()
                        && !app.session.native_sso_manual.is_empty()
                        && auth::callback_token_matches(
                            &app.session.native_sso_manual,
                            &token_param,
                        )
                    {
                        request_token = Some(app.session.native_sso_manual.clone());
                    }
                    let Some(rt) = request_token else {
                        app.save_session();
                        println!("login link stored, but no login token matches its token param - press Login in this app first");
                        std::process::exit(1);
                    };
                    match auth::exchange(&rt, &app.session.native_sso_etoken.clone(), &blob) {
                        Ok(bearer) => {
                            app.session.token = bearer;
                            app.session.frl_request.clear();
                            app.session.frl_blob.clear();
                            app.save_session();
                            println!("login token saved");
                        }
                        Err(e) => {
                            app.session.callback_attempted = true;
                            app.save_session();
                            println!("login token failed: {e}");
                            std::process::exit(1);
                        }
                    }
                }
            }
            None => println!(
                "no frl_login callback in this HAR — check Preserve log was on and login completed"
            ),
        }
        return true;
    }

    false
}

/// Flags that shape the window run.
#[allow(dead_code)]
pub struct WindowFlags {
    pub settings_page: bool,
    pub safe: bool,
    pub no_covers: bool,
    pub x11: bool,
}

pub fn window_flags(args: &[String]) -> WindowFlags {
    let settings_page = args.iter().any(|a| a == "--settings-page");
    // Low-VRAM or broken GL drivers segfault inside the graphics context
    // before any app code runs — the log stays empty because the window is
    // created before App::new. --safe forces software rendering; --no-covers
    // keeps the library grid light (plain grey cards instead of uploads).
    let safe = args.iter().any(|a| a == "--safe")
        || std::env::var("IRIS_SAFE")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    let no_covers = args.iter().any(|a| a == "--no-covers")
        || std::env::var("IRIS_NO_COVERS")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    let x11 = args.iter().any(|a| a == "--x11");
    WindowFlags {
        settings_page,
        safe,
        no_covers,
        x11,
    }
}

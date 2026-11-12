// Proton download machinery lives here, not in main.rs. Unpacks the chosen
// GE-Proton release into Steam's compatibilitytools.d (when Steam is present
// and the picker targets Steam) or into the app's own proton dir. All status
// strings come from the i18n tables.
#![cfg_attr(target_os = "windows", allow(unused_imports))]

use crate::fetcher;
use crate::i18n;
#[cfg(not(target_os = "windows"))]
use crate::steam_compat_dir;
#[cfg(not(target_os = "windows"))]
use crate::steam_present;
use crate::tr;
use crate::App;

/// Download the picked Proton release (threaded: synchronous HTTP would
/// freeze the window). Returns false when there was nothing to install yet.
#[cfg(not(target_os = "windows"))]
pub fn download(app: &mut App) -> bool {
    let Some((tag, name, url)) = app.proton_releases.get(app.proton_pick).cloned() else {
        app.status = tr(&app.config, "proton_pick_first");
        return false;
    };
    if app.install_inflight {
        app.status = tr(&app.config, "installer_busy");
        return false;
    }
    let dest = if app.proton_to_steam && steam_present() {
        steam_compat_dir()
    } else {
        if app.proton_to_steam {
            app.status = tr(&app.config, "proton_no_steam");
        }
        app.data_root().join("proton")
    };
    app.install_inflight = true;
    app.dl_inflight = true;
    app.dl_key.clear();
    app.dl_label = format!("Proton {tag}");
    app.dl_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    *app.dl_progress.lock().unwrap() = (0, 0, 0, 0);
    let size = tr(&app.config, "proton_size");
    app.status = i18n::fill(
        tr(&app.config, "install_tool_x").as_str(),
        &[("tag", &tag), ("size", size.as_str())],
    );
    crate::auth::log_line(&format!("proton install: {tag} -> {}", dest.display()));
    let slot = app.install_slot.clone();
    let repaint = app.ctx.clone();
    let prog = app.dl_progress.clone();
    let stop = app.dl_stop.clone();
    std::thread::spawn(move || {
        let res = fetcher::install_release(&url, &dest, &prog, &stop);
        *slot.lock().unwrap() =
            Some(res.map(|_| format!("{name} unpacked into {}", dest.display())));
        if let Some(c) = repaint {
            c.request_repaint();
        }
    });
    true
}

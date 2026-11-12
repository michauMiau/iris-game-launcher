// First-run wizard: one screen per setup task, each skippable from the small
// button in the top-right corner. The wizard is what a fresh install opens
// instead of the library grid — the old behavior (silently applying detected
// language/theme and showing an empty library) left new users without any
// pointer to what to configure.
//
// Layout rules: the counter sits in the same top row as the skip button;
// language and theme live on the first screen (they were already pre-filled
// from the desktop on first run, see App::new); important buttons and logo are
// centered; Next/Finish anchors to the bottom-right corner. Nested egui
// closures take immutable reads of app only; wizard writes go through local
// flags applied after the drawing closures end.

#[cfg(not(target_os = "windows"))]
use crate::detect_protons;
use crate::i18n;
#[cfg(target_os = "linux")]
use crate::steam_shortcuts;
use crate::tr;
use crate::{App, DirTarget};
use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Welcome,
    Handler,
    Login,
    Runtime,
    Games,
}

pub struct Onboarding {
    pub active: bool,
    pub step: Step,
    pub skipped: Vec<Step>,
}

impl Onboarding {
    pub fn new(active: bool) -> Self {
        Self {
            active,
            step: Step::Welcome,
            skipped: Vec::new(),
        }
    }

    pub fn steps() -> [Step; 5] {
        [
            Step::Welcome,
            Step::Handler,
            Step::Login,
            Step::Runtime,
            Step::Games,
        ]
    }

    pub fn index(&self) -> usize {
        Self::steps()
            .iter()
            .position(|s| *s == self.step)
            .unwrap_or(0)
    }

    pub fn is_last(&self) -> bool {
        self.index() + 1 >= Self::steps().len()
    }

    /// Advance leaves a step as done (never skipped); finish when past the
    /// last step. Callers keep config.setup_done and save_config in sync.
    pub fn advance(&mut self) -> bool {
        if self.is_last() {
            self.active = false;
            return true;
        }
        self.step = Self::steps()[self.index() + 1];
        false
    }

    pub fn skip_current(&mut self) -> bool {
        self.skipped.push(self.step);
        self.advance()
    }

    /// Back leaves the current step unfinished and returns to the previous
    /// screen; first screen has nothing to go back to (UI shows it disabled).
    pub fn back(&mut self) -> bool {
        if self.index() == 0 {
            return false;
        }
        self.step = Self::steps()[self.index() - 1];
        true
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn was_skipped(&self, step: Step) -> bool {
        self.skipped.contains(&step)
    }
}

#[derive(Clone, Copy)]
// DownloadRevive/ChooseProton arms switch by platform cfg; one target only
// constructs one of them, so dead-code stays quiet here.
#[allow(dead_code)]
enum Action {
    SetHandler,
    Login,
    CheckLogin,
    ApplyCallback,
    ChooseProton,
    DownloadRevive,
    ScanDirs,
    ImportFolder,
    SteamAddAll,
}

pub fn render(app: &mut App, ui: &mut egui::Ui) {
    // Skip is small, quiet and bare ("Skip"/"Pomiń"), in the top-right corner.
    // The counter shares the same row: push it right with a spacer instead of
    // a layout, so both sit at equal height.
    let mut skip = false;
    let step = app.onboarding.step;
    let n = app.onboarding.index() + 1;
    let total = Onboarding::steps().len();
    let skipped = app.onboarding.skipped.clone();
    let mut reopen: Option<Step> = None;
    let skip_w = 44.0;
    ui.horizontal(|ui| {
        ui.label(format!("{n}/{total}"));
        for s in skipped.iter().copied() {
            let label = format!(
                "{}: {}",
                step_name(s),
                tr(&app.config, "onboard_skipped_short")
            );
            if ui.label(egui::RichText::new(label).weak()).clicked() {
                reopen = Some(s);
            }
        }
        let space = (ui.available_width() - skip_w).max(0.0);
        ui.add_space(space);
        skip = ui.small_button(tr(&app.config, "onboard_skip")).clicked();
    });
    if let Some(s) = reopen {
        app.onboarding.step = s;
    }
    if skip {
        let done = app.onboarding.skip_current();
        app.status = tr(&app.config, "onboard_skipped");
        if done {
            app.config.setup_done = true;
            app.save_config();
        }
    }

    let mut action: Option<Action> = None;
    let mut next = false;
    let mut finish = false;
    // Buttons live right below the step content (the middle of the screen),
    // centered horizontally: a spacer inside the row places them at the
    // middle, not anchored to the bottom strip.
    let back_label = tr(&app.config, "onboard_back");
    let next_label = if step == Step::Games {
        tr(&app.config, "onboard_finish")
    } else {
        tr(&app.config, "onboard_next")
    };
    // Same width for Back and Next so they look identical; bold labels.
    let btn_w = back_label.chars().count().max(next_label.chars().count()) as f32 * 7.0 + 28.0;
    let pad = ((ui.available_width() - (btn_w * 2.0 + 12.0)) * 0.5).max(0.0);
    ui.vertical_centered(|ui| match step {
        Step::Welcome => {
            load_logo(app, ui);
            // Logo and title are larger than the library heading: the first
            // screen is the brand moment.
            let head = ui.style().text_styles[&egui::TextStyle::Heading].size;
            ui.label(
                egui::RichText::new("Iris Game Launcher")
                    .size(head * 1.5)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(env!("CARGO_PKG_VERSION").trim_end_matches(".0"))
                    .size(head * 0.55),
            );
            ui.label(tr(&app.config, "onboard_welcome_hint"));
            let mut pick_lang: Option<String> = None;
            let mut pick_theme: Option<String> = None;
            // The language/theme row lives centered too: a horizontal layout
            // would otherwise stretch across the whole panel and sit left.
            // Row width is estimated from the current language label lengths.
            let est = if app.config.lang == "pl" {
                250.0
            } else {
                220.0
            };
            let row_pad = ((ui.available_width() - est) * 0.5).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(row_pad);
                ui.label(tr(&app.config, "language"));
                let cur_lang = tr(&app.config, &format!("lang_{}", app.config.lang));
                ui.menu_button(cur_lang, |ui| {
                    for code in ["en", "pl"] {
                        if ui
                            .button(tr(&app.config, &format!("lang_{code}")))
                            .clicked()
                        {
                            pick_lang = Some(code.to_string());
                            ui.close_menu();
                        }
                    }
                });
                ui.label(tr(&app.config, "theme"));
                let cur_theme = tr(&app.config, &format!("theme_{}", app.config.theme));
                ui.menu_button(cur_theme, |ui| {
                    for th in ["dark", "light", "solarized"] {
                        if ui.button(tr(&app.config, &format!("theme_{th}"))).clicked() {
                            pick_theme = Some(th.to_string());
                            ui.close_menu();
                        }
                    }
                });
            });
            if let Some(lang) = pick_lang {
                app.config.lang = lang;
                app.save_config();
            }
            if let Some(theme) = pick_theme {
                app.config.theme = theme;
                app.save_config();
            }
        }
        Step::Handler => {
            ui.heading(tr(&app.config, "onboard_handler_head"));
            ui.label(tr(&app.config, "onboard_handler_hint"));
            ui.horizontal(|ui| {
                #[cfg(target_os = "linux")]
                {
                    let owner = crate::auth::handler_owner();
                    let label = if owner == "other" {
                        tr(&app.config, "handler_repoint")
                    } else {
                        tr(&app.config, "handler_set")
                    };
                    if owner != "launcher" && ui.button(label).clicked() {
                        action = Some(Action::SetHandler);
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    let label = if crate::auth::handler_owner() == "launcher" {
                        tr(&app.config, "handler_give_back")
                    } else {
                        tr(&app.config, "handler_take")
                    };
                    if ui.button(label).clicked() {
                        action = Some(Action::SetHandler);
                    }
                }
            });
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            ui.label(format!(
                "{}: {}",
                tr(&app.config, "login_link_handler_label"),
                crate::auth::handler_status(&app.config.lang)
            ));
        }
        Step::Login => {
            ui.heading(tr(&app.config, "onboard_login_head"));
            ui.label(tr(&app.config, "onboard_login_hint"));
            let login = ui.button(tr(&app.config, "login_btn")).clicked();
            let check = ui.button(tr(&app.config, "check_login")).clicked();
            ui.add(
                egui::TextEdit::singleline(&mut app.callback_url)
                    .hint_text(tr(&app.config, "login_link_hint"))
                    .desired_width(320.0),
            );
            let apply = ui.button(tr(&app.config, "apply_btn")).clicked();
            ui.label(if app.session.token.is_empty() {
                tr(&app.config, "onboard_login_none")
            } else {
                tr(&app.config, "onboard_login_present")
            });
            if login {
                action = Some(Action::Login);
            }
            if check {
                action = Some(Action::CheckLogin);
            }
            if apply {
                action = Some(Action::ApplyCallback);
            }
        }
        Step::Runtime => {
            ui.heading(tr(&app.config, "onboard_runtime_head"));
            #[cfg(not(target_os = "windows"))]
            {
                ui.label(tr(&app.config, "onboard_runtime_hint_linux"));
                let found = detect_protons(&app.config.proton_dirs);
                ui.label(format!(
                    "{}: {}",
                    tr(&app.config, "onboard_proton_found"),
                    found.len()
                ));
                if found.is_empty() {
                    ui.label(tr(&app.config, "no_proton_found"));
                }
                if ui
                    .button(tr(&app.config, "choose_proton_version"))
                    .clicked()
                {
                    action = Some(Action::ChooseProton);
                }
                let cur = app.config.proton_selected.clone();
                let mut pick: Option<String> = None;
                ui.horizontal(|ui| {
                    for p in &found {
                        if ui
                            .radio(!cur.is_empty() && cur == *p, crate::short_path(p))
                            .clicked()
                        {
                            pick = Some(p.clone());
                        }
                    }
                });
                if let Some(p) = pick {
                    app.config.proton_selected = p;
                    app.save_config();
                }
            }
            #[cfg(target_os = "windows")]
            {
                ui.label(tr(&app.config, "onboard_runtime_hint_windows"));
                if ui
                    .button(tr(&app.config, "onboard_revive_download"))
                    .clicked()
                {
                    action = Some(Action::DownloadRevive);
                }
                ui.add(
                    egui::TextEdit::singleline(&mut app.config.revive_dll)
                        .hint_text("LibOVRRT DLL path (ReviveNext install)")
                        .desired_width(320.0),
                );
            }
        }
        Step::Games => {
            ui.heading(tr(&app.config, "onboard_games_head"));
            ui.label(tr(&app.config, "onboard_games_hint"));
            let scan = ui.button(tr(&app.config, "onboard_scan_folders")).clicked();
            let import = ui.button(tr(&app.config, "import_game")).clicked();
            #[cfg(target_os = "linux")]
            {
                ui.label(tr(&app.config, "onboard_steam_hint"));
                if ui
                    .button(tr(&app.config, "onboard_steam_add_all"))
                    .clicked()
                {
                    action = Some(Action::SteamAddAll);
                }
            }
            ui.label(format!(
                "{}: {}",
                tr(&app.config, "library_count_label"),
                app.store.apps.len()
            ));
            if scan {
                action = Some(Action::ScanDirs);
            }
            if import {
                action = Some(Action::ImportFolder);
            }
        }
    });

    // Buttons live below the step content: spacer then Back then Next/Finish,
    // all centered horizontally on the screen. Back is inert on the first
    // screen.
    ui.add_space(16.0);
    let first = app.onboarding.index() == 0;
    let mut back = false;
    ui.horizontal(|ui| {
        ui.add_space(pad);
        let resp = ui.add(
            egui::Button::new(egui::RichText::new(back_label.clone()).strong())
                .min_size(egui::vec2(btn_w, 26.0)),
        );
        back = !first && resp.clicked();
        let resp = ui.add(
            egui::Button::new(egui::RichText::new(next_label.clone()).strong())
                .min_size(egui::vec2(btn_w, 26.0)),
        );
        if step == Step::Games {
            finish = resp.clicked();
        } else {
            next = resp.clicked();
        }
    });

    // Wizard actions run after the drawing closure so app can be borrowed
    // mutably for the handler/login/proton work.
    if let Some(a) = action {
        match a {
            Action::SetHandler => {
                #[cfg(target_os = "linux")]
                {
                    match crate::auth::install_oculus_handler(&app.config.lang) {
                        Ok(p) => {
                            app.status = i18n::fill(
                                tr(&app.config, "login_link_handler").as_str(),
                                &[("status", &p)],
                            )
                        }
                        Err(e) => {
                            app.status = i18n::fill(
                                tr(&app.config, "handler_install_failed").as_str(),
                                &[("msg", &e)],
                            )
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    let res = if crate::auth::handler_owner() == "launcher" {
                        crate::auth::uninstall_oculus_handler(&app.config.lang)
                    } else {
                        crate::auth::install_oculus_handler(&app.config.lang)
                    };
                    match res {
                        Ok(p) => {
                            app.status = i18n::fill(
                                tr(&app.config, "login_link_handler").as_str(),
                                &[("status", &p)],
                            )
                        }
                        Err(e) => {
                            app.status = i18n::fill(
                                tr(&app.config, "handler_change_failed").as_str(),
                                &[("msg", &e)],
                            )
                        }
                    }
                }
            }
            Action::Login => app.login_in_browser(),
            Action::CheckLogin => {
                if app.session.token.is_empty() {
                    app.status = "paste a login token first".into();
                } else {
                    app.start_probe();
                }
            }
            Action::ApplyCallback => app.redeem_callback(),
            #[cfg(not(target_os = "windows"))]
            Action::ChooseProton => {
                app.load_releases("GloriousEggroll/proton-ge-custom");
                app.proton_popup = true;
            }
            #[cfg(target_os = "windows")]
            Action::ChooseProton => {}
            #[cfg(target_os = "windows")]
            Action::DownloadRevive => app.download_revive_setup(),
            #[cfg(not(target_os = "windows"))]
            Action::DownloadRevive => {}
            Action::ScanDirs => app.rescan_dirs(),
            Action::ImportFolder => {
                let start = if !app.config.download_dir.is_empty() {
                    std::path::PathBuf::from(app.config.download_dir.trim())
                } else {
                    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
                };
                app.dir_popup = Some((start, DirTarget::Import));
            }
            #[cfg(target_os = "linux")]
            Action::SteamAddAll => steam_shortcuts::add_installed(app),
            #[cfg(not(target_os = "linux"))]
            Action::SteamAddAll => {}
        }
    }
    if next {
        app.onboarding.advance();
    }
    if back {
        app.onboarding.back();
    }
    if finish {
        app.onboarding.advance();
        app.config.setup_done = true;
        app.save_config();
    }
}

fn load_logo(app: &mut App, ui: &mut egui::Ui) {
    if app.logo_tex.is_none() {
        if let Ok(img) = image::load_from_memory(include_bytes!("../../assets/iris-logo.png")) {
            let rgba = img.to_rgba8();
            let w = rgba.width() as usize;
            let h = rgba.height() as usize;
            let ci = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba.into_raw());
            app.logo_tex = Some(ui.ctx().load_texture(
                "iris-logo",
                ci,
                egui::TextureOptions::default(),
            ));
            app.logo_aspect = w as f32 / h as f32;
        }
    }
    if let Some(tex) = &app.logo_tex {
        let side = ui.style().text_styles[&egui::TextStyle::Heading].size * 3.4;
        ui.add(
            egui::Image::new(tex)
                .fit_to_exact_size(egui::vec2(side * app.logo_aspect.max(1.0), side)),
        );
    }
}

pub fn step_name(step: Step) -> &'static str {
    match step {
        Step::Welcome => "welcome",
        Step::Handler => "handler",
        Step::Login => "login",
        Step::Runtime => "runtime",
        Step::Games => "games",
    }
}

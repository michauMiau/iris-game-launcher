use crate::onboarding::{Onboarding, Step};
use crate::steam_shortcuts::{desktop_content, percent_encode, slug};
use crate::Config;

#[test]
fn wizard_steps_advance_in_order_and_finish_at_games() {
    let mut w = Onboarding::new(true);
    assert_eq!(w.step, Step::Welcome);
    assert!(!w.advance());
    assert_eq!(w.step, Step::Appearance);
    assert!(!w.advance());
    assert_eq!(w.step, Step::Handler);
    assert!(!w.advance());
    assert_eq!(w.step, Step::Login);
    assert!(!w.advance());
    assert_eq!(w.step, Step::Runtime);
    assert!(!w.advance());
    assert_eq!(w.step, Step::Games);
    assert!(w.is_last());
    assert!(w.advance());
    assert!(!w.active);
}

#[test]
fn skipping_a_step_records_it_and_stays_reopenable() {
    let mut w = Onboarding::new(true);
    w.skip_current();
    assert!(w.was_skipped(Step::Welcome));
    assert_eq!(w.step, Step::Appearance);
    w.skipped.push(Step::Handler);
    assert_eq!(w.index(), 1);
}

#[test]
fn steam_launcher_file_points_at_the_binary_and_quotes_name() {
    let content = desktop_content("/opt/iris/IrisLauncher", "Bone Work & Co");
    assert!(content.contains("[Desktop Entry]"));
    assert!(content.contains("Exec=\"/opt/iris/IrisLauncher\" --launch \"Bone Work & Co\""));
    assert!(content.contains("Name=Bone Work & Co"));
    assert!(!content.contains("Icon="));
    assert!(content.contains("Terminal=false"));
}

#[test]
fn slug_keeps_shortcut_filenames_shell_safe() {
    assert_eq!(slug("Bone Work"), "bone-work");
    assert_eq!(slug("A  B--C"), "a-b-c");
    assert_eq!(slug("Bonelab 2.19!!"), "bonelab-2-19");
    assert_eq!(slug("  spaced  "), "spaced");
}

#[test]
fn steam_url_quotes_everything_non_alphanumeric() {
    let enc = percent_encode("/home user/steam-shortcuts/bone-work.desktop");
    assert!(enc.contains("%20"));
    assert!(enc.contains("%2F"));
    assert!(enc.starts_with("%2F"));
    assert!(enc.contains(".desktop") || enc.contains("%2E"));
}

#[test]
fn configs_written_by_old_versions_treat_setup_done_as_done() {
    // serde default for a missing field is true: existing configs from older
    // builds must not reopen the wizard.
    let cfg: Config = serde_json::from_str(r#"{"download_dir":"/tmp/x"}"#).unwrap();
    assert!(cfg.setup_done);
    // A fresh Config::default() (no config.json yet) opens the wizard.
    assert!(!Config::default().setup_done);
}

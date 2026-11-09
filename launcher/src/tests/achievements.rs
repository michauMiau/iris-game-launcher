use crate::achievements;

#[test]
fn parse_unlock() {
    let t = "foo\novr_Achievements_Unlock ach1\nbar\novr_Achievements_Unlock ach2\novr_Achievements_Unlock ach1\n";
    let v = achievements::parse_unlock_lines(t);
    assert_eq!(v, vec!["ach1".to_string(), "ach2".to_string()]);
}

#[test]
fn save_load_roundtrip() {
    let dir = std::env::temp_dir().join("iris-ach-test");
    let _ = std::fs::remove_dir_all(&dir);
    achievements::save(&dir, "app", &["a".into(), "b".into()]).unwrap();
    let loaded = achievements::load(&dir, "app");
    assert_eq!(loaded.len(), 2);
}

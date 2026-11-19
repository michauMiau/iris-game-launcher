use crate::achievements::{self, Achievement};

#[test]
fn local_unlocks_reads_the_always_on_log() {
    let dir = std::env::temp_dir().join("iris-ach-local");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("iris-achievements.log"),
        "ACH_ONE\nACH_TWO\nACH_ONE\n\n",
    )
    .unwrap();
    let v = achievements::local_unlocks(&dir);
    assert_eq!(v, vec!["ACH_ONE".to_string(), "ACH_TWO".to_string()]);
}

#[test]
fn save_load_roundtrip() {
    let dir = std::env::temp_dir().join("iris-ach-test");
    let _ = std::fs::remove_dir_all(&dir);
    let achs = vec![
        Achievement {
            api_name: "a".into(),
            description: "Desc A".into(),
            unlocked: true,
        },
        Achievement {
            api_name: "b".into(),
            description: String::new(),
            unlocked: false,
        },
    ];
    achievements::save(&dir, "app", &achs).unwrap();
    let loaded = achievements::load(&dir, "app");
    assert_eq!(loaded, achs);
}

#[test]
fn apply_local_marks_and_adds() {
    let mut achs = vec![
        Achievement {
            api_name: "a".into(),
            description: "A".into(),
            unlocked: false,
        },
        Achievement {
            api_name: "b".into(),
            description: "B".into(),
            unlocked: false,
        },
    ];
    achievements::apply_local(&mut achs, &["a".to_string(), "c".to_string()]);
    assert!(achs.iter().find(|x| x.api_name == "a").unwrap().unlocked);
    assert!(!achs.iter().find(|x| x.api_name == "b").unwrap().unlocked);
    assert!(achs.iter().any(|x| x.api_name == "c" && x.unlocked));
}

#[test]
fn merge_server_marks_unlocked_and_dedups() {
    let defs = vec![
        ("a".to_string(), "A".to_string()),
        ("b".to_string(), "B".to_string()),
    ];
    let merged = achievements::merge_server(
        defs,
        &["a".to_string()],
        &["b".to_string(), "c".to_string()],
    );
    assert!(merged.iter().find(|x| x.api_name == "a").unwrap().unlocked);
    assert!(merged.iter().find(|x| x.api_name == "b").unwrap().unlocked);
    assert!(merged.iter().any(|x| x.api_name == "c" && x.unlocked));
    assert_eq!(merged.len(), 3);
}

#[test]
fn old_id_name_files_migrate() {
    let dir = std::env::temp_dir().join("iris-ach-migrate");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("achievements")).unwrap();
    std::fs::write(
        dir.join("achievements/app.json"),
        r#"{"achievements":[{"id":"old","name":"Old desc","icon":"x"}]}"#,
    )
    .unwrap();
    let loaded = achievements::load(&dir, "app");
    assert_eq!(loaded[0].api_name, "old");
    assert_eq!(loaded[0].description, "Old desc");
    assert!(!loaded[0].unlocked);
}

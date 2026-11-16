use crate::i18n::*;
#[test]
fn polish_and_fallback() {
    assert_eq!(text(Lang::Pl, "install"), "Zainstaluj");
    assert_eq!(text(Lang::En, "install"), "Install");
    // Unknown key falls back to the key itself, never empty.
    assert_eq!(text(Lang::Pl, "not_a_key"), "not_a_key");
    assert_eq!(
        fill(&text(Lang::En, "downloading_x"), &[("name", "Bonelab")]),
        "downloading Bonelab"
    );
    assert_eq!(
        fill(&text(Lang::Pl, "update_none"), &[]),
        "brak aktualizacji — to najnowsza wersja"
    );
}

#[test]
fn every_english_key_has_a_polish_translation() {
    let missing: Vec<&str> = EN
        .iter()
        .filter(|(key, _)| !PL.iter().any(|(k, _)| k == key))
        .map(|(key, _)| *key)
        .collect();
    assert!(
        missing.is_empty(),
        "Polish table lacks: {}",
        missing.join(", ")
    );
}
#[test]
fn polish_table_has_no_orphan_keys() {
    let orphans: Vec<&str> = PL
        .iter()
        .filter(|(key, _)| !EN.iter().any(|(k, _)| k == key))
        .map(|(key, _)| *key)
        .collect();
    assert!(
        orphans.is_empty(),
        "Polish keys that English never asks for: {}",
        orphans.join(", ")
    );
}
#[test]
fn translation_tables_have_no_duplicate_keys() {
    for table in [EN, PL] {
        for (i, (key, _)) in table.iter().enumerate() {
            assert!(
                !table[i + 1..].iter().any(|(k, _)| k == key),
                "duplicate key {key}"
            );
        }
    }
}
#[test]
fn every_tr_key_exists_in_the_english_table() {
    let missing: Vec<String> = used_static_keys()
        .into_iter()
        .filter(|k| !EN.iter().any(|(key, _)| *key == k.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "tr() asks keys no table defines: {:?}",
        missing
    );
}
#[test]
fn theme_and_language_dropdown_keys_exist() {
    for k in [
        "theme_dark",
        "theme_light",
        "theme_solarized",
        "lang_en",
        "lang_pl",
    ] {
        assert!(EN.iter().any(|(key, _)| *key == k), "missing {k}");
        assert!(PL.iter().any(|(key, _)| *key == k), "missing {k} in Polish");
    }
}

/// forms (theme_dark, lang_en...) are covered by the tables themselves.
fn used_static_keys() -> Vec<String> {
    let src = include_str!("../main.rs");
    let mut keys: Vec<String> = Vec::new();
    for pattern in ["tr(&self.config, \"", "tr(&config, \""] {
        let mut rest = src;
        while let Some(pos) = rest.find(pattern) {
            rest = &rest[pos + pattern.len()..];
            let Some(q2) = rest.find('"') else { break };
            if !rest[..q2].is_empty() {
                keys.push(rest[..q2].to_string());
            }
        }
    }
    keys
}

/// Keys assembled at runtime from a prefix: the theme menu formats
/// `theme_{th}` and the language menu formats `lang_{code}`, so the tables
/// hold one entry per suffix while the code only ever names the prefix. A
/// table key counts as used when production code mentions the literal, or
/// when it starts with one of these dynamically formatted prefixes.
const DYNAMIC_KEY_PREFIXES: &[&str] = &["theme_", "lang_"];

/// Production UI code: `src/*.rs` minus `i18n.rs` (it `include!`s the tables,
/// so every key would otherwise look used) and minus the `tests/` tree (tests
/// exercise lookups; they are not call sites).
fn production_sources() -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut src = String::new();
    for entry in std::fs::read_dir(&root).expect("src dir") {
        let path = entry.expect("src entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("i18n.rs") {
            continue;
        }
        src.push_str(&std::fs::read_to_string(&path).expect("source"));
        src.push('\n');
    }
    src
}

fn used_keys() -> std::collections::HashSet<String> {
    let src = production_sources();
    let dynamic: Vec<&str> = DYNAMIC_KEY_PREFIXES
        .iter()
        .copied()
        .filter(|p| src.contains(&format!("\"{p}{{")))
        .collect();
    let mut used = std::collections::HashSet::new();
    for table in all_tables() {
        for (key, _) in table.1 {
            if src.contains(&format!("\"{key}\"")) || dynamic.iter().any(|p| key.starts_with(p)) {
                used.insert(key.to_string());
            }
        }
    }
    used
}

/// Every shipped language table. Adding a language means adding it here too,
/// so the orphan check covers it.
fn all_tables() -> Vec<(&'static str, &'static [(&'static str, &'static str)])> {
    vec![("en", EN), ("pl", PL)]
}

#[test]
fn no_language_table_entry_is_orphaned() {
    let used = used_keys();
    for (label, table) in all_tables() {
        let orphans: Vec<&str> = table
            .iter()
            .filter(|(key, _)| !used.contains(*key))
            .map(|(key, _)| *key)
            .collect();
        assert!(
            orphans.is_empty(),
            "{label} translation entries never referenced by code: {:?}",
            orphans
        );
    }
}

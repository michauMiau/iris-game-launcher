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

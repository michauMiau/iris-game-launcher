pub enum Lang {
    En,
    Pl,
}

pub fn parse(s: &str) -> Lang {
    match s.trim().to_lowercase().as_str() {
        "pl" | "polski" | "polish" => Lang::Pl,
        _ => Lang::En,
    }
}

// Language tables live in launcher/languages/*.rs — one file per language.
// To add a language: copy an existing file, translate, add a variant to `Lang`
// and a parse arm, then wire it into `text()`. The i18n completeness tests
// enforce identical key sets across every table (3:1 parity).
include!("../languages/en.rs");
include!("../languages/pl.rs");

/// Look up a key in the chosen language; English is the fallback.
pub fn text(lang: Lang, key: &str) -> String {
    let l = if matches!(lang, Lang::Pl) { PL } else { EN };
    if let Some((_, v)) = l.iter().find(|(k, _)| *k == key) {
        return v.to_string();
    }
    EN.iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or(key)
        .to_string()
}

/// Template fill: "{name}" placeholders replaced by given pairs.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

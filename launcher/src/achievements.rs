use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Achievement {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub icon: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    achievements: Vec<Achievement>,
}

pub fn parse_unlock_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("ovr_Achievements_Unlock") {
            let id = rest.trim();
            if !id.is_empty() && !out.contains(&id.to_string()) {
                out.push(id.to_string());
            }
        }
    }
    out
}

pub fn store_path(data_dir: &Path, app_id: &str) -> PathBuf {
    data_dir.join("achievements").join(format!("{app_id}.json"))
}

pub fn load(data_dir: &Path, app_id: &str) -> Vec<Achievement> {
    let p = store_path(data_dir, app_id);
    let Ok(s) = fs::read_to_string(&p) else {
        return Vec::new();
    };
    let Ok(store): Result<Store, _> = serde_json::from_str(&s) else {
        return Vec::new();
    };
    store.achievements
}

pub fn save(data_dir: &Path, app_id: &str, ids: &[String]) -> std::io::Result<()> {
    let dir = data_dir.join("achievements");
    fs::create_dir_all(&dir)?;
    // Names/icons from Meta-side metadata survive a re-save: entries that
    // were recorded earlier keep their fetched metadata instead of being
    // rewritten as bare ids.
    let known = load(data_dir, app_id);
    let mut entries: Vec<Achievement> = Vec::new();
    for id in ids {
        if let Some(old) = known.iter().find(|a| &a.id == id) {
            entries.push(old.clone());
        } else {
            entries.push(Achievement {
                id: id.clone(),
                name: String::new(),
                icon: String::new(),
            });
        }
    }
    let store = Store {
        achievements: entries,
    };
    let json = serde_json::to_string_pretty(&store).unwrap_or_default();
    fs::write(store_path(data_dir, app_id), json)
}

/// Scan trace logs in game dir and data dir, collect unlock ids, persist.
pub fn collect(data_dir: &Path, game_dir: Option<&Path>) -> Vec<String> {
    let mut ids = Vec::new();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(gd) = game_dir {
        // Both shim traces live beside the exe: the platform shim's file and
        // the plugin shim's distinct file.
        candidates.push(gd.join("iris-shim-trace.log"));
        candidates.push(gd.join("iris-ovrplugin-trace.log"));
    }
    candidates.push(data_dir.join("iris-shim-trace.log"));
    // also any in data dir root? scan one level
    if let Ok(entries) = fs::read_dir(data_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file()
                && p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.contains("trace"))
                    .unwrap_or(false)
            {
                candidates.push(p);
            }
        }
    }
    for c in candidates {
        if let Ok(text) = fs::read_to_string(&c) {
            for id in parse_unlock_lines(&text) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids
}

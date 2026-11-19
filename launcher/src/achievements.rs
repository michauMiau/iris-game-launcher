use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Achievement {
    #[serde(alias = "id")]
    pub api_name: String,
    #[serde(default, alias = "name")]
    pub description: String,
    #[serde(default)]
    pub unlocked: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    achievements: Vec<Achievement>,
}

pub fn store_path(data_dir: &Path, app_id: &str) -> PathBuf {
    data_dir.join("achievements").join(format!("{app_id}.json"))
}

/// Always-on unlock log the platform shim writes beside the game exe,
/// independent of the opt-in trace radio.
pub fn unlock_log_path(game_dir: &Path) -> PathBuf {
    game_dir.join("iris-achievements.log")
}

pub fn load(data_dir: &Path, app_id: &str) -> Vec<Achievement> {
    let Ok(s) = fs::read_to_string(store_path(data_dir, app_id)) else {
        return Vec::new();
    };
    serde_json::from_str::<Store>(&s)
        .map(|st| st.achievements)
        .unwrap_or_default()
}

pub fn save(data_dir: &Path, app_id: &str, achs: &[Achievement]) -> std::io::Result<()> {
    let dir = data_dir.join("achievements");
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(&Store {
        achievements: achs.to_vec(),
    })
    .unwrap_or_default();
    fs::write(store_path(data_dir, app_id), json)
}

/// Read the shim's always-on unlock log: one api_name per line, deduped.
pub fn local_unlocks(game_dir: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(unlock_log_path(game_dir)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let id = line.trim();
        if !id.is_empty() && !out.contains(&id.to_string()) {
            out.push(id.to_string());
        }
    }
    out
}

/// Mark cached achievements unlocked from the local log; add local unlocks
/// that have no definition yet (offline play before a server fetch).
pub fn apply_local(achs: &mut Vec<Achievement>, local: &[String]) {
    let set: HashSet<&str> = local.iter().map(|s| s.as_str()).collect();
    for a in achs.iter_mut() {
        if set.contains(a.api_name.as_str()) {
            a.unlocked = true;
        }
    }
    for id in local {
        if !achs.iter().any(|a| &a.api_name == id) {
            achs.push(Achievement {
                api_name: id.clone(),
                description: String::new(),
                unlocked: true,
            });
        }
    }
}

/// Build the display list from server definitions plus the unlocked sets.
pub fn merge_server(
    defs: Vec<(String, String)>,
    server_unlocked: &[String],
    local: &[String],
) -> Vec<Achievement> {
    let unlocked: HashSet<&str> = server_unlocked
        .iter()
        .chain(local.iter())
        .map(|s| s.as_str())
        .collect();
    let mut out: Vec<Achievement> = defs
        .into_iter()
        .map(|(api, desc)| Achievement {
            unlocked: unlocked.contains(api.as_str()),
            api_name: api,
            description: desc,
        })
        .collect();
    for id in local.iter().chain(server_unlocked.iter()) {
        if !out.iter().any(|a| &a.api_name == id) {
            out.push(Achievement {
                api_name: id.clone(),
                description: String::new(),
                unlocked: true,
            });
        }
    }
    out
}

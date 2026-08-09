//! Card database from HearthstoneJSON (compact BG index under `data/cards/`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

const SYNC_URL: &str = "https://api.hearthstonejson.com/v1/latest/enUS/cards.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDef {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub card_type: Option<String>,
    #[serde(rename = "techLevel")]
    pub tech_level: Option<u32>,
    pub attack: Option<i32>,
    pub health: Option<i32>,
    pub race: Option<String>,
    pub races: Option<Vec<String>>,
    #[serde(default)]
    pub mechanics: Vec<String>,
    #[serde(rename = "referencedTags", default)]
    pub referenced_tags: Vec<String>,
    pub text: Option<String>,
    #[serde(rename = "isBattlegroundsPoolMinion")]
    pub is_pool_minion: Option<bool>,
    #[serde(rename = "battlegroundsHero")]
    pub battlegrounds_hero: Option<bool>,
    pub cost: Option<i32>,
}

impl CardDef {
    pub fn has_mechanic(&self, m: &str) -> bool {
        self.mechanics.iter().any(|x| x == m)
            || self.referenced_tags.iter().any(|x| x == m)
    }

    pub fn is_deathrattle(&self) -> bool {
        self.has_mechanic("DEATHRATTLE")
    }

    pub fn is_divine_shield(&self) -> bool {
        self.has_mechanic("DIVINE_SHIELD")
    }

    pub fn is_taunt(&self) -> bool {
        self.has_mechanic("TAUNT")
    }

    pub fn is_reborn(&self) -> bool {
        self.has_mechanic("REBORN")
    }

    pub fn is_poisonous(&self) -> bool {
        self.has_mechanic("POISONOUS") || self.has_mechanic("VENOMOUS")
    }

    pub fn is_windfury(&self) -> bool {
        self.has_mechanic("WINDFURY") || self.has_mechanic("MEGA_WINDFURY")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CardFile {
    source: String,
    locale: String,
    #[serde(default)]
    build: Option<String>,
    cards: Vec<CardDef>,
}

#[derive(Debug, Default)]
pub struct CardDb {
    by_id: HashMap<String, CardDef>,
    pub source_path: Option<PathBuf>,
    pub count: usize,
}

impl CardDb {
    pub fn get(&self, id: &str) -> Option<&CardDef> {
        self.by_id.get(id)
    }

    pub fn name(&self, id: &str) -> Option<&str> {
        self.get(id).and_then(|c| c.name.as_deref())
    }

    pub fn load_file(path: &Path) -> anyhow::Result<Self> {
        let text = fs::read_to_string(path)?;
        let file: CardFile = serde_json::from_str(&text)?;
        let mut by_id = HashMap::with_capacity(file.cards.len());
        for c in file.cards {
            by_id.insert(c.id.clone(), c);
        }
        let count = by_id.len();
        Ok(Self {
            by_id,
            source_path: Some(path.to_path_buf()),
            count,
        })
    }

    pub fn empty() -> Self {
        Self::default()
    }
}

static DB: OnceLock<RwLock<CardDb>> = OnceLock::new();

fn db_lock() -> &'static RwLock<CardDb> {
    DB.get_or_init(|| RwLock::new(CardDb::empty()))
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("BGC_CARDS") {
        v.push(PathBuf::from(p));
    }
    v.push(PathBuf::from("data/cards/bg_enUS.json"));
    v.push(PathBuf::from("../data/cards/bg_enUS.json"));
    v.push(PathBuf::from("../../data/cards/bg_enUS.json"));
    // from crate dir when running tests
    v.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/cards/bg_enUS.json"));
    v
}

/// Load card DB from disk if not already loaded (or empty).
pub fn ensure_card_data_loaded() {
    let lock = db_lock();
    {
        let g = lock.read().unwrap();
        if g.count > 0 {
            return;
        }
    }
    for p in candidate_paths() {
        if p.is_file() {
            if let Ok(db) = CardDb::load_file(&p) {
                let mut g = lock.write().unwrap();
                *g = db;
                return;
            }
        }
    }
}

pub fn card_db_loaded() -> bool {
    db_lock().read().unwrap().count > 0
}

pub fn card_db_count() -> usize {
    db_lock().read().unwrap().count
}

pub fn card_name(id: &str) -> Option<String> {
    ensure_card_data_loaded();
    db_lock().read().unwrap().name(id).map(|s| s.to_string())
}

pub fn card_def(id: &str) -> Option<CardDef> {
    ensure_card_data_loaded();
    db_lock().read().unwrap().get(id).cloned()
}

/// Keep Battlegrounds-relevant rows from full HearthstoneJSON `cards.json`.
pub fn filter_battlegrounds_cards(raw: &[serde_json::Value]) -> Vec<CardDef> {
    let mut out = Vec::new();
    for v in raw {
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
        let set = v.get("set").and_then(|x| x.as_str()).unwrap_or("");
        if set != "BATTLEGROUNDS" && !id.starts_with("BG") && !id.starts_with("TB_Bacon") {
            continue;
        }
        if let Ok(c) = serde_json::from_value::<CardDef>(v.clone()) {
            out.push(c);
        }
    }
    out
}

/// Download latest cards.json and write compact `data/cards/bg_enUS.json`.
pub fn sync_card_db(out_dir: &Path) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(out_dir)?;
    let agent = ureq::AgentBuilder::new()
        .timeout_read(std::time::Duration::from_secs(120))
        .build();
    let resp = agent.get(SYNC_URL).call().map_err(|e| anyhow::anyhow!("download: {e}"))?;
    let text = resp
        .into_string()
        .map_err(|e| anyhow::anyhow!("read body: {e}"))?;
    let raw: Vec<serde_json::Value> = serde_json::from_str(&text)?;
    let cards = filter_battlegrounds_cards(&raw);
    let file = CardFile {
        source: "hearthstonejson".into(),
        locale: "enUS".into(),
        build: Some("latest".into()),
        cards,
    };
    let out = out_dir.join("bg_enUS.json");
    fs::write(&out, serde_json::to_string(&file)?)?;
    // hot-reload into process
    let db = CardDb::load_file(&out)?;
    *db_lock().write().unwrap() = db;
    Ok(out)
}

/// Deathrattle summons table (optional overlay on top of card defs).
pub use deathrattles::deathrattle_summons;

mod deathrattles {
    use serde::Deserialize;
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    use std::sync::OnceLock;

    #[derive(Debug, Clone, Deserialize)]
    pub struct DeathrattleSummon {
        pub card_id: String,
        #[serde(default)]
        pub name: Option<String>,
        pub atk: i32,
        pub health: i32,
        #[serde(default = "one")]
        pub count: u8,
    }

    fn one() -> u8 {
        1
    }

    static DEATHRATTLES: OnceLock<HashMap<String, Vec<DeathrattleSummon>>> = OnceLock::new();

    fn load_map() -> HashMap<String, Vec<DeathrattleSummon>> {
        let candidates = [
            Path::new("data/combat/deathrattles.json"),
            Path::new("../data/combat/deathrattles.json"),
            Path::new("../../data/combat/deathrattles.json"),
        ];
        for p in candidates {
            if let Ok(text) = fs::read_to_string(p) {
                if let Ok(raw) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text)
                {
                    let mut map = HashMap::new();
                    for (k, v) in raw {
                        if !v.is_array() {
                            continue;
                        }
                        if let Ok(list) = serde_json::from_value::<Vec<DeathrattleSummon>>(v) {
                            map.insert(k, list);
                        }
                    }
                    return map;
                }
            }
        }
        HashMap::new()
    }

    pub fn deathrattle_summons(card_id: &str) -> &'static [DeathrattleSummon] {
        let map = DEATHRATTLES.get_or_init(load_map);
        map.get(card_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_bundled_bg_index() {
        ensure_card_data_loaded();
        assert!(card_db_count() > 1000, "count={}", card_db_count());
        assert_eq!(card_name("BG35_814").as_deref(), Some("Scarlet Survivor"));
    }
}

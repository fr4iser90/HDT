//! Optional curated Battlegrounds hero tier list (local JSON).

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct HeroTierFile {
    /// Free-form attribution, e.g. "manual · patch 33.x".
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub tiers: Vec<HeroTierBucket>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeroTierBucket {
    pub tier: String,
    #[serde(default)]
    pub heroes: Vec<HeroTierEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeroTierEntry {
    pub card_id: String,
    #[serde(default)]
    pub name: String,
    /// Optional avg place / note.
    #[serde(default)]
    pub note: String,
}

fn tiers_path() -> PathBuf {
    if let Ok(p) = std::env::var("BGC_HERO_TIERS") {
        return PathBuf::from(p);
    }
    // Prefer repo data/ when running from source.
    let candidates = [
        PathBuf::from("data/meta/hero_tiers.json"),
        PathBuf::from("../data/meta/hero_tiers.json"),
        PathBuf::from("../../data/meta/hero_tiers.json"),
    ];
    for c in candidates {
        if c.is_file() {
            return c;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let xdg = PathBuf::from(home)
            .join(".config")
            .join("bgc")
            .join("hero_tiers.json");
        if xdg.is_file() {
            return xdg;
        }
    }
    PathBuf::from("data/meta/hero_tiers.json")
}

static TIERS: OnceLock<Option<HeroTierFile>> = OnceLock::new();

pub fn load_hero_tiers() -> Option<&'static HeroTierFile> {
    TIERS
        .get_or_init(|| {
            let path = tiers_path();
            let Ok(bytes) = fs::read(&path) else {
                return None;
            };
            serde_json::from_slice(&bytes).ok()
        })
        .as_ref()
}

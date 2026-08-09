//! Incremental Power.log line parser → high-level events.
//!
//! Prefers `GameState.DebugPrint*` over `PowerTaskList` to avoid duplicate spam.
//! Tracks FULL_ENTITY / SHOW_ENTITY blocks so nested `tag=` lines bind to an entity id.

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogEvent {
    CreateGame,
    GameType { value: i32 },
    PlayerInfo { player_id: u32, name: String },
    TagChange {
        entity: String,
        tag: String,
        value: String,
    },
    /// Nested tag inside a FULL_ENTITY/SHOW_ENTITY block (bound to entity id).
    EntityTag {
        entity_id: i32,
        tag: String,
        value: String,
    },
    FullEntity {
        entity_id: i32,
        card_id: Option<String>,
    },
    ShowEntity {
        entity_id: Option<i32>,
        card_id: Option<String>,
    },
    FindGameState { state: String },
    ScreenMode { mode: String },
}

pub struct PowerParser {
    tag_change: Regex,
    game_type: Regex,
    full_entity: Regex,
    show_entity: Regex,
    show_entity_num: Regex,
    player_info: Regex,
    nested_tag: Regex,
    /// Entity id of the current FULL_ENTITY / SHOW_ENTITY block being defined.
    block_entity_id: Option<i32>,
}

impl Default for PowerParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerParser {
    pub fn new() -> Self {
        Self {
            tag_change: Regex::new(
                r"TAG_CHANGE\s+Entity=(?P<entity>.+?)\s+tag=(?P<tag>\S+)\s+value=(?P<value>\S+)",
            )
            .expect("regex"),
            game_type: Regex::new(r"tag=GAME_TYPE\s+value=(?P<value>\d+)").expect("regex"),
            full_entity: Regex::new(
                r"FULL_ENTITY\s+-\s+(?:Creating|Updating).*?\bID=(?P<id>\d+)(?:\s+CardID=(?P<card>\S+))?",
            )
            .expect("regex"),
            show_entity: Regex::new(
                r"SHOW_ENTITY\s+-\s+Updating\s+Entity=(?P<entity>.+?)\s+CardID=(?P<card>\S+)",
            )
            .expect("regex"),
            show_entity_num: Regex::new(
                r"SHOW_ENTITY\s+-\s+Updating\s+Entity=(?P<id>\d+)\s+CardID=(?P<card>\S+)",
            )
            .expect("regex"),
            player_info: Regex::new(r"PlayerID=(?P<id>\d+),\s*PlayerName=(?P<name>.+)$")
                .expect("regex"),
            nested_tag: Regex::new(r"DebugPrintPower\(\)\s+-\s+tag=(?P<tag>\S+)\s+value=(?P<value>\S+)")
                .expect("regex"),
            block_entity_id: None,
        }
    }

    pub fn parse_line(&mut self, line: &str) -> Option<LogEvent> {
        let line = line.trim_end();
        if line.is_empty() {
            return None;
        }

        let is_game_state = line.contains("GameState.DebugPrint");
        let is_task_list = line.contains("PowerTaskList.DebugPrint");
        // PowerTaskList is a delayed mirror — never drive state from it (incl. CREATE_GAME).
        if is_task_list && !is_game_state {
            self.block_entity_id = None;
            return None;
        }
        if !is_game_state {
            return None;
        }

        // Only GameState CREATE_GAME starts a match (not PowerTaskList duplicates).
        if line.contains("CREATE_GAME") {
            self.block_entity_id = None;
            return Some(LogEvent::CreateGame);
        }

        // Nested tags belong to the open FULL_ENTITY/SHOW_ENTITY block
        if let Some(caps) = self.nested_tag.captures(line) {
            if let Some(id) = self.block_entity_id {
                let tag = caps["tag"].to_string();
                if is_entity_tag(&tag) {
                    return Some(LogEvent::EntityTag {
                        entity_id: id,
                        tag,
                        value: caps["value"].trim_end_matches('\r').to_string(),
                    });
                }
            }
            // fall through — might be GAME_TYPE etc.
        }

        if let Some(caps) = self.player_info.captures(line) {
            self.block_entity_id = None;
            let player_id: u32 = caps["id"].parse().ok()?;
            let name = caps["name"].trim().to_string();
            if !name.is_empty() && name != "UNKNOWN HUMAN PLAYER" {
                return Some(LogEvent::PlayerInfo { player_id, name });
            }
        }

        if let Some(caps) = self.game_type.captures(line) {
            let value: i32 = caps["value"].parse().unwrap_or(-1);
            return Some(LogEvent::GameType { value });
        }

        if let Some(caps) = self.full_entity.captures(line) {
            let id: i32 = caps["id"].parse().ok()?;
            self.block_entity_id = Some(id);
            let card_id = caps.name("card").map(|m| {
                let c = m.as_str().trim_end_matches('\r');
                c.to_string()
            });
            let card_id = card_id.filter(|c| !c.is_empty());
            return Some(LogEvent::FullEntity {
                entity_id: id,
                card_id,
            });
        }

        if let Some(caps) = self.show_entity_num.captures(line) {
            let id: i32 = caps["id"].parse().ok()?;
            self.block_entity_id = Some(id);
            return Some(LogEvent::ShowEntity {
                entity_id: Some(id),
                card_id: Some(caps["card"].trim_end_matches('\r').to_string()),
            });
        }

        if let Some(caps) = self.show_entity.captures(line) {
            let entity = caps["entity"].to_string();
            let id = entity_id(&entity).or_else(|| entity.parse().ok());
            self.block_entity_id = id;
            return Some(LogEvent::ShowEntity {
                entity_id: id,
                card_id: Some(caps["card"].trim_end_matches('\r').to_string()),
            });
        }

        if let Some(caps) = self.tag_change.captures(line) {
            self.block_entity_id = None;
            let tag = caps["tag"].to_string();
            if is_interesting_tag(&tag) || is_entity_tag(&tag) {
                return Some(LogEvent::TagChange {
                    entity: caps["entity"].trim().to_string(),
                    tag,
                    value: caps["value"].trim_end_matches('\r').to_string(),
                });
            }
            return None;
        }

        // Closing the block on unrelated GameState lines
        if line.contains("BLOCK_") || line.contains("DebugPrintOptions") {
            self.block_entity_id = None;
        }

        None
    }
}

fn is_interesting_tag(tag: &str) -> bool {
    matches!(
        tag,
        "RESOURCES"
            | "RESOURCES_USED"
            | "HEALTH"
            | "DAMAGE"
            | "ARMOR"
            | "PLAYER_TECH_LEVEL"
            | "PLAYERLEADERBOARDPLACE"
            | "PLAYER_LEADERBOARD_PLACE"
            | "TURN"
            | "NEXT_STEP"
            | "STEP"
            | "ZONE"
            | "ZONE_POSITION"
            | "CONTROLLER"
            | "HERO_ENTITY"
            | "CURRENT_PLAYER"
            | "PLAYSTATE"
            | "MULLIGAN_STATE"
            | "GAME_TYPE"
            | "FORMAT_TYPE"
            | "SCENARIO_ID"
            | "BACON_MAX_PLAYER_TECH_LEVEL"
            | "CARDTYPE"
            | "ATK"
            | "TECH_LEVEL"
            | "IS_BACON_POOL_MINION"
            | "HAS_DRAG_TO_BUY"
            | "DIVINE_SHIELD"
            | "TAUNT"
            | "REBORN"
            | "POISONOUS"
            | "VENOMOUS"
            | "WINDFURY"
            | "MEGA_WINDFURY"
            | "BOARD_VISUAL_STATE"
            | "NEXT_OPPONENT_PLAYER_ID"
            | "NEXT_OPPONENT_TEAMMATE_PLAYER_ID"
    ) || tag.starts_with("BACON_")
}

fn is_entity_tag(tag: &str) -> bool {
    matches!(
        tag,
        "ZONE"
            | "ZONE_POSITION"
            | "CONTROLLER"
            | "CARDTYPE"
            | "ATK"
            | "HEALTH"
            | "TECH_LEVEL"
            | "IS_BACON_POOL_MINION"
            | "HAS_DRAG_TO_BUY"
            | "COST"
            | "DAMAGE"
            | "ARMOR"
            | "DIVINE_SHIELD"
            | "TAUNT"
            | "REBORN"
            | "POISONOUS"
            | "VENOMOUS"
            | "WINDFURY"
            | "MEGA_WINDFURY"
    ) || tag.starts_with("BACON_")
}

pub fn entity_card_id(entity: &str) -> Option<&str> {
    let key = "cardId=";
    let idx = entity.find(key)?;
    let rest = &entity[idx + key.len()..];
    let end = rest.find(|c: char| c.is_whitespace() || c == ']').unwrap_or(rest.len());
    let id = &rest[..end];
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

pub fn entity_player(entity: &str) -> Option<u32> {
    let key = "player=";
    let idx = entity.find(key)?;
    let rest = &entity[idx + key.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

pub fn entity_id(entity: &str) -> Option<i32> {
    if let Ok(n) = entity.parse::<i32>() {
        return Some(n);
    }
    let key = " id=";
    let idx = entity.find(key)?;
    let rest = &entity[idx + key.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

pub fn entity_name(entity: &str) -> Option<String> {
    let key = "entityName=";
    let idx = entity.find(key)?;
    let rest = &entity[idx + key.len()..];
    let end = rest.find(" id=").unwrap_or(rest.len());
    let name = rest[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub fn is_hero_card_id(card_id: &str) -> bool {
    if card_id.ends_with('p')
        || card_id.contains("_HP_")
        || card_id.contains("BaconShop_HP")
        || card_id.contains("_HERO_Buddy")
    {
        return false;
    }
    // Prefer card DB when loaded (excludes Kerrigan tokens like BG31_HERO_811t4)
    if crate::cards::card_db_loaded() {
        if let Some(def) = crate::cards::card_def(card_id) {
            return def.card_type.as_deref() == Some("HERO")
                || def.battlegrounds_hero == Some(true);
        }
        // Unknown id while DB is loaded: don't guess from substring
        return card_id.contains("BaconShop_HERO_") && !card_id.contains("_t");
    }
    if card_id.contains("BaconShop_HERO_") {
        return !card_id.contains("_t");
    }
    // BG31_HERO_811 or BG31_HERO_811_SKIN_A — not …t4 / enchantments
    is_bg_numbered_hero_id(card_id)
}

fn is_bg_numbered_hero_id(card_id: &str) -> bool {
    // BG{digits}_HERO_{digits} optional _SKIN_{alnum}
    let bytes = card_id.as_bytes();
    if bytes.len() < 10 || !card_id.starts_with("BG") {
        return false;
    }
    let Some(hero_at) = card_id.find("_HERO_") else {
        return false;
    };
    let after = &card_id[hero_at + 6..];
    let num_len = after.chars().take_while(|c| c.is_ascii_digit()).count();
    if num_len == 0 {
        return false;
    }
    let rest = &after[num_len..];
    rest.is_empty() || rest.starts_with("_SKIN_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_entity_block_tags() {
        let mut p = PowerParser::new();
        let ev = p
            .parse_line(
                "D 12:00:00.0 GameState.DebugPrintPower() - FULL_ENTITY - Creating ID=283 CardID=BG35_814",
            )
            .unwrap();
        assert!(matches!(ev, LogEvent::FullEntity { entity_id: 283, .. }));
        let ev = p
            .parse_line(
                "D 12:00:00.0 GameState.DebugPrintPower() -         tag=CONTROLLER value=7",
            )
            .unwrap();
        assert_eq!(
            ev,
            LogEvent::EntityTag {
                entity_id: 283,
                tag: "CONTROLLER".into(),
                value: "7".into()
            }
        );
        let ev = p
            .parse_line(
                "D 12:00:00.0 GameState.DebugPrintPower() -         tag=CARDTYPE value=MINION",
            )
            .unwrap();
        assert!(matches!(ev, LogEvent::EntityTag { tag, .. } if tag == "CARDTYPE"));
    }

    #[test]
    fn skips_power_task_list() {
        let mut p = PowerParser::new();
        assert_eq!(
            p.parse_line(
                "D 12:00:00.0 PowerTaskList.DebugPrintPower() - TAG_CHANGE Entity=GameEntity tag=TURN value=7"
            ),
            None
        );
        assert_eq!(
            p.parse_line(
                "D 12:00:00.0 PowerTaskList.DebugPrintPower() -     CREATE_GAME"
            ),
            None
        );
        assert!(matches!(
            p.parse_line("D 12:00:00.0 GameState.DebugPrintPower() - CREATE_GAME"),
            Some(LogEvent::CreateGame)
        ));
    }

    #[test]
    fn hero_card_filter() {
        assert!(is_hero_card_id("TB_BaconShop_HERO_10"));
        assert!(is_hero_card_id("BG31_HERO_811"));
        assert!(is_hero_card_id("BG22_HERO_000_SKIN_E")); // Ribbon Tavish skin
        assert!(!is_hero_card_id("BG35_HERO_001p"));
        assert!(!is_hero_card_id("BG31_HERO_811t4")); // Hydralisk token
        assert!(!is_hero_card_id("BG31_HERO_811t4_G"));
    }
}

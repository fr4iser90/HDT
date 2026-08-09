//! Entity map built from Power.log FULL_ENTITY / TAG_CHANGE / SHOW_ENTITY.

use crate::parser::{entity_card_id, entity_id, entity_name, entity_player, is_hero_card_id};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Entity {
    pub id: i32,
    pub card_id: Option<String>,
    pub name: Option<String>,
    pub zone: Option<String>,
    pub zone_pos: Option<i32>,
    pub controller: Option<u32>,
    pub player: Option<u32>,
    pub card_type: Option<String>,
    pub atk: Option<i32>,
    pub health: Option<i32>,
    pub tech_level: Option<u32>,
    pub is_bacon_pool_minion: bool,
    pub has_drag_to_buy: bool,
    pub divine_shield: bool,
    pub taunt: bool,
    pub reborn: bool,
    pub poisonous: bool,
    pub windfury: bool,
    pub mega_windfury: bool,
    /// Battlegrounds lobby / standings seat (1 = top).
    pub leaderboard_place: Option<u32>,
}

impl Entity {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    pub fn apply_tag(&mut self, tag: &str, value: &str) {
        match tag {
            "ZONE" => {
                self.zone = Some(value.to_string());
                if value != "PLAY" {
                    self.has_drag_to_buy = false;
                }
            }
            "ZONE_POSITION" => self.zone_pos = value.parse().ok(),
            "CONTROLLER" => {
                self.controller = value.parse().ok();
                // GameState often never sets player=; CONTROLLER is the authority.
                if self.player.is_none() {
                    self.player = self.controller;
                }
            }
            "CARDTYPE" => self.card_type = Some(value.to_string()),
            "ATK" => self.atk = value.parse().ok(),
            "HEALTH" => self.health = value.parse().ok(),
            "TECH_LEVEL" => self.tech_level = value.parse().ok(),
            "IS_BACON_POOL_MINION" => self.is_bacon_pool_minion = value == "1",
            "HAS_DRAG_TO_BUY" => self.has_drag_to_buy = value == "1",
            "DIVINE_SHIELD" => self.divine_shield = value == "1",
            "TAUNT" => self.taunt = value == "1",
            "REBORN" => self.reborn = value == "1",
            "POISONOUS" | "VENOMOUS" => self.poisonous = value == "1",
            "WINDFURY" => self.windfury = value == "1",
            "MEGA_WINDFURY" => self.mega_windfury = value == "1",
            "CARD_ID" | "CardID" => self.card_id = Some(value.to_string()),
            "PLAYERLEADERBOARDPLACE" | "PLAYER_LEADERBOARD_PLACE" => {
                self.leaderboard_place = value.parse().ok();
            }
            _ => {}
        }
    }

    pub fn merge_from_entity_blob(&mut self, blob: &str) {
        if let Some(c) = entity_card_id(blob) {
            self.card_id = Some(c.to_string());
        }
        if let Some(n) = entity_name(blob) {
            self.name = Some(n);
        }
        if let Some(p) = entity_player(blob) {
            self.player = Some(p);
        }
        // zone=PLAY inside blob
        if let Some(idx) = blob.find("zone=") {
            let rest = &blob[idx + 5..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == ']')
                .unwrap_or(rest.len());
            self.zone = Some(rest[..end].to_string());
        }
        if let Some(idx) = blob.find("zonePos=") {
            let rest = &blob[idx + 8..];
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '-')
                .unwrap_or(rest.len());
            self.zone_pos = rest[..end].parse().ok();
        }
    }

    pub fn is_minion(&self) -> bool {
        if self.card_type.as_deref() == Some("MINION") {
            return true;
        }
        self.card_id.as_deref().is_some_and(|c| {
            c.starts_with("BG")
                && !is_hero_card_id(c)
                && !c.contains("Trinket")
                && !c.contains("Button")
                && self.card_type.as_deref() != Some("ENCHANTMENT")
                && self.card_type.as_deref() != Some("HERO_POWER")
                && self.card_type.as_deref() != Some("SPELL")
                && self.card_type.as_deref() != Some("BATTLEGROUND_SPELL")
                && self.card_type.as_deref() != Some("BATTLEGROUND_TRINKET")
                && self.card_type.as_deref() != Some("GAME_MODE_BUTTON")
        })
    }

    pub fn is_tavern_spell(&self) -> bool {
        matches!(
            self.card_type.as_deref(),
            Some("BATTLEGROUND_SPELL") | Some("SPELL")
        ) || self.card_id.as_deref().is_some_and(|c| {
            // common tavern spells e.g. BG28_810 Tavern Coin
            c.starts_with("BG") && self.card_type.as_deref() != Some("MINION") && self.has_drag_to_buy
        })
    }

    pub fn short_label(&self) -> String {
        let card = self.card_id.as_deref().unwrap_or("?");
        let name = self
            .name
            .clone()
            .filter(|n| !n.is_empty() && n != "?")
            .or_else(|| crate::cards::card_name(card))
            .unwrap_or_else(|| "?".into());
        match (self.atk, self.health) {
            (Some(a), Some(h)) => format!("{name}({card} {a}/{h})"),
            _ => format!("{name}({card})"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityStore {
    pub entities: HashMap<i32, Entity>,
    /// Player id of Bartender Bob (shop side), if known.
    pub bob_player_id: Option<u32>,
}

impl EntityStore {
    pub fn clear(&mut self) {
        self.entities.clear();
        self.bob_player_id = None;
    }

    pub fn upsert(&mut self, id: i32) -> &mut Entity {
        self.entities.entry(id).or_insert_with(|| Entity::new(id))
    }

    pub fn apply_full_entity(&mut self, id: i32, card_id: Option<String>) {
        let e = self.upsert(id);
        if let Some(c) = card_id {
            if c == "TB_BaconShopBob" {
                // bob player filled later via player=/CONTROLLER
            }
            e.card_id = Some(c);
        }
    }

    pub fn apply_tag_on_id(&mut self, id: i32, tag: &str, value: &str) {
        {
            let e = self.upsert(id);
            e.apply_tag(tag, value);
        }
        self.maybe_set_bob(id);
    }

    pub fn apply_tag_on_blob(&mut self, blob: &str, tag: &str, value: &str) {
        let Some(id) = entity_id(blob).or_else(|| blob.parse().ok()) else {
            return;
        };
        {
            let e = self.upsert(id);
            e.merge_from_entity_blob(blob);
            e.apply_tag(tag, value);
        }
        self.maybe_set_bob(id);
    }

    fn maybe_set_bob(&mut self, id: i32) {
        if let Some(e) = self.entities.get(&id) {
            if e.card_id.as_deref() == Some("TB_BaconShopBob") {
                if let Some(p) = e.player.or(e.controller) {
                    self.bob_player_id = Some(p);
                }
            }
        }
    }

    /// Current tavern offerings: live buy targets (`HAS_DRAG_TO_BUY` in PLAY).
    ///
    /// Prefer this over "everything on Bob" — old shop copies and combat ghosts
    /// often linger with ZONE=PLAY / CONTROLLER=Bob without being buyable.
    pub fn shop(&self, friendly_player: Option<u32>) -> Vec<&Entity> {
        let bob = self.bob_player_id.or_else(|| self.infer_bob_player(friendly_player));
        let items = self
            .entities
            .values()
            .filter(|e| e.has_drag_to_buy)
            .filter(|e| e.zone.as_deref() == Some("PLAY"))
            .filter(|e| {
                // Prefer Bob's controller; fall back to "not friendly" if Bob unknown
                match bob {
                    Some(b) => e.controller == Some(b) || e.player == Some(b),
                    None => friendly_player.is_none_or(|fp| {
                        e.controller != Some(fp) && e.player != Some(fp)
                    }),
                }
            })
            .filter(|e| self.is_shop_card(e));
        // One offering per slot (newest id wins)
        let mut best: HashMap<i32, &Entity> = HashMap::new();
        for e in items {
            let pos = e.zone_pos.unwrap_or(e.id);
            match best.get(&pos) {
                Some(prev) if prev.id >= e.id => {}
                _ => {
                    best.insert(pos, e);
                }
            }
        }
        let mut items: Vec<&Entity> = best.into_values().collect();
        items.sort_by_key(|e| e.zone_pos.unwrap_or(0));
        items
    }

    pub fn infer_bob_player(&self, friendly_player: Option<u32>) -> Option<u32> {
        self.entities.values().find_map(|e| {
            if e.card_id.as_deref() == Some("TB_BaconShopBob") {
                e.player.or(e.controller)
            } else {
                None
            }
        })
        .or_else(|| {
            // Fallback: most common player id among HAS_DRAG_TO_BUY in PLAY, not friendly
            self.entities.values().find_map(|e| {
                if e.has_drag_to_buy && e.zone.as_deref() == Some("PLAY") {
                    let p = e.player.or(e.controller)?;
                    if friendly_player != Some(p) {
                        return Some(p);
                    }
                }
                None
            })
        })
    }

    fn is_shop_card(&self, e: &Entity) -> bool {
        if e.card_id.as_deref().is_some_and(|c| {
            c.contains("BaconShop")
                || c.contains("Button")
                || c.contains("DragBuy")
                || c.contains("DragSell")
                || c.contains("Trinket")
                || is_hero_card_id(c)
        }) {
            return false;
        }
        matches!(
            e.card_type.as_deref(),
            Some("MINION") | Some("SPELL") | Some("BATTLEGROUND_SPELL")
        ) || e.is_minion()
            || (e.card_id.as_ref().is_some_and(|c| c.starts_with("BG"))
                && e.card_type.as_deref() != Some("ENCHANTMENT")
                && e.card_type.as_deref() != Some("HERO")
                && e.card_type.as_deref() != Some("HERO_POWER")
                && e.card_type.as_deref() != Some("GAME_MODE_BUTTON")
                && e.card_type.as_deref() != Some("BATTLEGROUND_TRINKET")
                && e.card_type.as_deref() != Some("MOVE_MINION_HOVER_TARGET"))
    }

    /// Friendly board minions in PLAY (recruit + combat ghosts until they leave).
    pub fn board(&self, friendly_player: Option<u32>) -> Vec<&Entity> {
        let Some(fp) = friendly_player else {
            return Vec::new();
        };
        let bob = self.bob_player_id.or_else(|| self.infer_bob_player(Some(fp)));
        let mut items: Vec<&Entity> = self
            .entities
            .values()
            .filter(|e| e.zone.as_deref() == Some("PLAY"))
            .filter(|e| e.player == Some(fp) || e.controller == Some(fp))
            .filter(|e| bob.is_none_or(|b| e.player != Some(b) && e.controller != Some(b)))
            .filter(|e| !e.has_drag_to_buy)
            .filter(|e| e.zone_pos.unwrap_or(0) >= 1)
            .filter(|e| e.card_type.as_deref() == Some("MINION") || e.is_minion())
            .filter(|e| {
                !e.card_id.as_deref().is_some_and(|c| {
                    c.contains("BaconShop")
                        || c.contains("Button")
                        || c.contains("Trinket")
                        || is_hero_card_id(c)
                })
            })
            .collect();
        // Deduplicate by zone_pos: keep highest entity id (newest) per slot.
        // Combat/token spam often leaves multiple entities claiming the same slot.
        let mut best: HashMap<i32, &Entity> = HashMap::new();
        for e in items.drain(..) {
            let pos = e.zone_pos.unwrap_or(0);
            match best.get(&pos) {
                Some(prev) if prev.id >= e.id => {}
                _ => {
                    best.insert(pos, e);
                }
            }
        }
        let mut items: Vec<&Entity> = best.into_values().collect();
        items.sort_by_key(|e| e.zone_pos.unwrap_or(0));
        items
    }

    /// Combat-side minions for a controller (friendly or Bob/opponent combat copies).
    pub fn combat_minions(&self, controller: u32) -> Vec<&Entity> {
        let mut items: Vec<&Entity> = self
            .entities
            .values()
            .filter(|e| e.zone.as_deref() == Some("PLAY"))
            .filter(|e| e.controller == Some(controller) || e.player == Some(controller))
            .filter(|e| !e.has_drag_to_buy)
            .filter(|e| e.zone_pos.unwrap_or(0) >= 1)
            .filter(|e| {
                e.card_type.as_deref() == Some("MINION")
                    || (e.is_minion() && e.atk.unwrap_or(0) > 0)
            })
            .filter(|e| e.health.unwrap_or(0) > 0)
            .filter(|e| {
                !e.card_id.as_deref().is_some_and(|c| {
                    c.contains("BaconShop")
                        || c.contains("Button")
                        || c.contains("Trinket")
                        || is_hero_card_id(c)
                })
            })
            .collect();
        let mut best: HashMap<i32, &Entity> = HashMap::new();
        for e in items.drain(..) {
            let pos = e.zone_pos.unwrap_or(0);
            match best.get(&pos) {
                Some(prev) if prev.id >= e.id => {}
                _ => {
                    best.insert(pos, e);
                }
            }
        }
        let mut items: Vec<&Entity> = best.into_values().collect();
        items.sort_by_key(|e| e.zone_pos.unwrap_or(0));
        items
    }

    pub fn hand(&self, friendly_player: Option<u32>) -> Vec<&Entity> {
        let Some(fp) = friendly_player else {
            return Vec::new();
        };
        let mut items: Vec<&Entity> = self
            .entities
            .values()
            .filter(|e| e.zone.as_deref() == Some("HAND"))
            .filter(|e| e.player == Some(fp) || e.controller == Some(fp))
            .filter(|e| {
                e.card_id.as_ref().is_some_and(|c| {
                    !is_hero_card_id(c)
                        && (c.starts_with("BG")
                            || e.card_type.as_deref() == Some("MINION")
                            || e.card_type.as_deref() == Some("SPELL")
                            || e.card_type.as_deref() == Some("BATTLEGROUND_SPELL"))
                })
            })
            .collect();
        items.sort_by_key(|e| e.zone_pos.unwrap_or(0));
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shop_only_buyable_in_play() {
        let mut s = EntityStore::default();
        s.apply_full_entity(1, Some("BG35_814".into()));
        s.apply_tag_on_id(1, "CONTROLLER", "15");
        s.apply_tag_on_id(1, "CARDTYPE", "MINION");
        s.apply_tag_on_id(1, "ZONE", "PLAY");
        s.apply_tag_on_id(1, "ZONE_POSITION", "1");
        s.apply_tag_on_id(1, "HAS_DRAG_TO_BUY", "1");

        // Stale shop ghost without drag flag
        s.apply_full_entity(2, Some("BG32_236".into()));
        s.apply_tag_on_id(2, "CONTROLLER", "15");
        s.apply_tag_on_id(2, "CARDTYPE", "MINION");
        s.apply_tag_on_id(2, "ZONE", "PLAY");
        s.apply_tag_on_id(2, "ZONE_POSITION", "2");

        let shop = s.shop(Some(7));
        assert_eq!(shop.len(), 1);
        assert_eq!(shop[0].id, 1);

        s.apply_tag_on_id(1, "HAS_DRAG_TO_BUY", "0");
        s.apply_tag_on_id(1, "ZONE", "REMOVEDFROMGAME");
        assert!(s.shop(Some(7)).is_empty());
    }

    #[test]
    fn board_dedupes_zone_pos() {
        let mut s = EntityStore::default();
        s.apply_full_entity(10, Some("BG36_200".into()));
        s.apply_tag_on_id(10, "CONTROLLER", "7");
        s.apply_tag_on_id(10, "CARDTYPE", "MINION");
        s.apply_tag_on_id(10, "ZONE", "PLAY");
        s.apply_tag_on_id(10, "ZONE_POSITION", "1");

        s.apply_full_entity(20, Some("BG36_200_G".into()));
        s.apply_tag_on_id(20, "CONTROLLER", "7");
        s.apply_tag_on_id(20, "CARDTYPE", "MINION");
        s.apply_tag_on_id(20, "ZONE", "PLAY");
        s.apply_tag_on_id(20, "ZONE_POSITION", "1");

        let board = s.board(Some(7));
        assert_eq!(board.len(), 1);
        assert_eq!(board[0].id, 20);
    }
}

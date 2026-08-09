//! Normalized live game state for the MVP spike.

use crate::cards::ensure_card_data_loaded;
use crate::combat::{simulate, CombatInput, CombatOdds, FightMode, SimMinion};
use crate::entities::EntityStore;
use crate::parser::{
    entity_card_id, entity_id, entity_name, entity_player, is_hero_card_id, LogEvent,
};
use serde::{Deserialize, Serialize};

/// Hearthstone GAME_TYPE values we care about (subset).
pub const GAME_TYPE_BATTLEGROUNDS: i32 = 23;
pub const GAME_TYPE_BATTLEGROUNDS_FRIENDLY: i32 = 24;
pub const GAME_TYPE_BATTLEGROUNDS_DUO: i32 = 37;
pub const GAME_TYPE_BATTLEGROUNDS_DUO_VS_AI: i32 = 38;
pub const GAME_TYPE_BATTLEGROUNDS_DUO_FRIENDLY: i32 = 39;
pub const GAME_TYPE_BATTLEGROUNDS_DUO_AI_VS_AI: i32 = 40;
pub const GAME_TYPE_BATTLEGROUNDS_DUO_1_PLAYER_VS_AI: i32 = 41;
const COMBAT_TRIALS: u32 = 500;

pub fn is_battlegrounds_game_type(v: i32) -> bool {
    matches!(
        v,
        GAME_TYPE_BATTLEGROUNDS
            | GAME_TYPE_BATTLEGROUNDS_FRIENDLY
            | 35
            | 36 // AI variants
            | GAME_TYPE_BATTLEGROUNDS_DUO
            | GAME_TYPE_BATTLEGROUNDS_DUO_VS_AI
            | GAME_TYPE_BATTLEGROUNDS_DUO_FRIENDLY
            | GAME_TYPE_BATTLEGROUNDS_DUO_AI_VS_AI
            | GAME_TYPE_BATTLEGROUNDS_DUO_1_PLAYER_VS_AI
    )
}

pub fn is_battlegrounds_duo_game_type(v: i32) -> bool {
    matches!(
        v,
        GAME_TYPE_BATTLEGROUNDS_DUO
            | GAME_TYPE_BATTLEGROUNDS_DUO_VS_AI
            | GAME_TYPE_BATTLEGROUNDS_DUO_FRIENDLY
            | GAME_TYPE_BATTLEGROUNDS_DUO_AI_VS_AI
            | GAME_TYPE_BATTLEGROUNDS_DUO_1_PLAYER_VS_AI
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    #[default]
    Unknown,
    Battlegrounds,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    #[default]
    Idle,
    Searching,
    Connecting,
    InMatch,
}

/// Coarse “where am I” for the overlay status panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchPhase {
    NoGame,
    Menu,
    Queuing,
    HeroDraft,
    Tavern,
    Combat,
    DuoSpectate,
}

impl MatchPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoGame => "No game",
            Self::Menu => "Menu / lobby",
            Self::Queuing => "Queuing",
            Self::HeroDraft => "Hero draft",
            Self::Tavern => "Tavern",
            Self::Combat => "Combat",
            Self::DuoSpectate => "Duo — partner fight",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpponentSnapshot {
    pub name: Option<String>,
    pub board: Vec<String>,
    pub tavern: Option<u32>,
    pub last_turn: Option<u32>,
    /// Lobby seat from `PLAYER_LEADERBOARD_PLACE` (1 = top of left strip).
    pub place: Option<u32>,
    pub player_id: Option<u32>,
    pub hero_card_id: Option<String>,
    pub hero_name: Option<String>,
    pub health: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LiveState {
    pub mode: GameMode,
    pub queue: QueueStatus,
    pub game_type: Option<i32>,
    pub turn: Option<u32>,
    pub gold: Option<u32>,
    pub health: Option<i32>,
    pub damage: Option<i32>,
    pub armor: Option<i32>,
    pub tavern_tier: Option<u32>,
    pub player_name: Option<String>,
    pub player_id: Option<u32>,
    pub hero_entity_id: Option<i32>,
    pub hero_card_id: Option<String>,
    pub hero_name: Option<String>,
    /// Heroes offered in draft (card ids), local player only.
    pub draft_offers: Vec<String>,
    pub board: Vec<String>,
    pub shop: Vec<String>,
    pub hand: Vec<String>,
    /// Lobby is Battlegrounds Duos (sequential 1v1 fights).
    pub is_duo: bool,
    pub in_combat: bool,
    pub combat_opponent: Option<String>,
    /// Player id of the upcoming / current fight target (`NEXT_OPPONENT_PLAYER_ID`).
    pub next_opponent_player_id: Option<u32>,
    /// Player id of the opponent we just fought (set when combat ends).
    pub last_fight_player_id: Option<u32>,
    pub combat_damage_cap: Option<i32>,
    pub opponent_tavern: Option<u32>,
    /// True when the local player is the active combatant (not just spectating partner in Duo).
    pub our_fight: bool,
    pub combat_odds: Option<CombatOdds>,
    pub last_opponent: Option<OpponentSnapshot>,
    /// Last known board per opponent (lobby tracker).
    pub opponent_boards: Vec<OpponentSnapshot>,
    pub known_players: Vec<(u32, String)>,
    /// Current Power.log size (bytes); watcher updates this.
    #[serde(default)]
    pub power_log_bytes: u64,
    /// True when Power.log has not grown recently (HS log feed stuck).
    #[serde(default)]
    pub power_log_stale: bool,
    #[serde(skip)]
    pub entities: EntityStore,
    /// Last stable recruit board (used when combat copies lag).
    #[serde(skip)]
    pre_combat_us: Vec<SimMinion>,
    /// Odds locked for the current fight (don't re-sim as minions die mid-combat).
    #[serde(skip)]
    combat_odds_locked: bool,
    #[serde(skip)]
    combat_fp_fingerprint: String,
    /// First non-empty enemy combat board this fight (ignore later DR summons).
    #[serde(skip)]
    opponent_board_snapshotted: bool,
    pub last_event: Option<String>,
    pub create_game_count: u32,
    pub lines_seen: u64,
    pub interesting_events: u64,
}

impl LiveState {
    fn apply_game_type(&mut self, value: i32) {
        self.game_type = Some(value);
        if is_battlegrounds_game_type(value) {
            self.mode = GameMode::Battlegrounds;
            if is_battlegrounds_duo_game_type(value) {
                self.is_duo = true;
            }
        } else {
            self.mode = GameMode::Other;
        }
    }

    /// Best-effort phase for HUD status.
    pub fn match_phase(&self) -> MatchPhase {
        match self.queue {
            QueueStatus::Searching | QueueStatus::Connecting => return MatchPhase::Queuing,
            _ => {}
        }
        if self.mode != GameMode::Battlegrounds && self.queue != QueueStatus::InMatch {
            if self.lines_seen == 0 {
                return MatchPhase::NoGame;
            }
            return MatchPhase::Menu;
        }
        if self.in_combat {
            if self.is_duo && !self.our_fight {
                return MatchPhase::DuoSpectate;
            }
            return MatchPhase::Combat;
        }
        // Draft: offered heroes, no locked friendly hero yet.
        if !self.draft_offers.is_empty() && self.hero_card_id.is_none() {
            return MatchPhase::HeroDraft;
        }
        if self.queue == QueueStatus::InMatch || self.mode == GameMode::Battlegrounds {
            if !self.draft_offers.is_empty() && self.hero_card_id.is_none() {
                return MatchPhase::HeroDraft;
            }
            return MatchPhase::Tavern;
        }
        MatchPhase::Menu
    }

    /// Our lobby seat (1 = top), if known from leaderboard tags.
    pub fn our_place(&self) -> Option<u32> {
        let pid = self.player_id?;
        self.opponent_boards
            .iter()
            .find(|o| o.player_id == Some(pid))
            .and_then(|o| o.place)
            .or_else(|| {
                self.hero_card_id.as_ref().and_then(|hid| {
                    self.opponent_boards
                        .iter()
                        .find(|o| o.hero_card_id.as_deref() == Some(hid.as_str()))
                        .and_then(|o| o.place)
                })
            })
    }

    pub fn is_next_opponent(&self, op: &OpponentSnapshot) -> bool {
        if let Some(pid) = self.next_opponent_player_id {
            if op.player_id == Some(pid) {
                return true;
            }
        }
        if self.in_combat {
            if let Some(ref n) = self.combat_opponent {
                return op.name.as_deref() == Some(n.as_str())
                    || op.hero_name.as_deref() == Some(n.as_str());
            }
        }
        false
    }

    pub fn is_last_fight_opponent(&self, op: &OpponentSnapshot) -> bool {
        if let Some(pid) = self.last_fight_player_id {
            if op.player_id == Some(pid) {
                return true;
            }
        }
        if let Some(ref last) = self.last_opponent {
            if last.player_id.is_some() && last.player_id == op.player_id {
                return true;
            }
            if let Some(ref n) = last.name {
                return op.name.as_deref() == Some(n.as_str())
                    || op.hero_name.as_deref() == Some(n.as_str());
            }
        }
        false
    }

    pub fn apply(&mut self, event: &LogEvent) {
        self.interesting_events += 1;
        self.last_event = Some(format!("{event:?}"));

        match event {
            LogEvent::CreateGame => {
                self.create_game_count += 1;
                self.turn = None;
                self.gold = None;
                self.health = None;
                self.damage = None;
                self.armor = None;
                self.tavern_tier = None;
                self.hero_entity_id = None;
                self.hero_card_id = None;
                self.hero_name = None;
                self.player_id = None;
                self.player_name = None;
                self.draft_offers.clear();
                self.board.clear();
                self.shop.clear();
                self.hand.clear();
                self.entities.clear();
                self.in_combat = false;
                self.combat_odds = None;
                self.combat_opponent = None;
                self.next_opponent_player_id = None;
                self.last_fight_player_id = None;
                self.combat_damage_cap = None;
                self.opponent_tavern = None;
                self.our_fight = false;
                self.combat_fp_fingerprint.clear();
                self.pre_combat_us.clear();
                self.combat_odds_locked = false;
                self.opponent_board_snapshotted = false;
                self.known_players.clear();
                self.last_opponent = None;
                self.opponent_boards.clear();
                self.is_duo = false;
                self.queue = QueueStatus::InMatch;
                ensure_card_data_loaded();
            }
            LogEvent::GameType { value } => {
                self.apply_game_type(*value);
            }
            LogEvent::PlayerInfo { player_id, name } => {
                if !self
                    .known_players
                    .iter()
                    .any(|(id, n)| id == player_id && n == name)
                {
                    self.known_players.push((*player_id, name.clone()));
                }
                // Duo lobby: multiple battletags (or BACON_DUO tags later)
                let humanish = self
                    .known_players
                    .iter()
                    .filter(|(_, n)| n.contains('#') || (!n.is_empty() && n != "UNKNOWN HUMAN PLAYER"))
                    .count();
                if humanish >= 2 {
                    // still may be solo lobby listing — duo confirmed via BACON_DUO_* tags
                }
                if self.player_name.is_none() {
                    self.player_name = Some(name.clone());
                    self.player_id = Some(*player_id);
                } else if name.contains('#')
                    && self
                        .player_name
                        .as_ref()
                        .is_some_and(|n| !n.contains('#'))
                {
                    self.player_name = Some(name.clone());
                    self.player_id = Some(*player_id);
                }
            }
            LogEvent::FindGameState { state } => {
                self.queue = map_find_game_state(state);
            }
            LogEvent::ScreenMode { mode } => match mode.as_str() {
                "BACON" | "BACON_COLLECTION" => {
                    self.mode = GameMode::Battlegrounds;
                    if self.queue == QueueStatus::InMatch {
                        self.queue = QueueStatus::Idle;
                    }
                }
                "BACON_PARTY" => {
                    self.mode = GameMode::Battlegrounds;
                    self.is_duo = true;
                    if self.queue == QueueStatus::InMatch {
                        self.queue = QueueStatus::Idle;
                    }
                }
                "HUB" | "LOGIN" => {
                    self.mode = GameMode::Unknown;
                    self.queue = QueueStatus::Idle;
                }
                "GAMEPLAY" => {
                    self.queue = QueueStatus::InMatch;
                }
                _ => {}
            },
            LogEvent::TagChange { entity, tag, value } => {
                self.entities.apply_tag_on_blob(entity, tag, value);
                self.apply_tag(entity, tag, value);
                self.refresh_lists();
            }
            LogEvent::EntityTag {
                entity_id,
                tag,
                value,
            } => {
                self.entities.apply_tag_on_id(*entity_id, tag, value);
                // Hero HP/armor sometimes arrive as bare Entity=114 tags
                if Some(*entity_id) == self.hero_entity_id {
                    match tag.as_str() {
                        "HEALTH" => self.health = value.parse().ok(),
                        "DAMAGE" => self.damage = value.parse().ok(),
                        "ARMOR" => self.armor = value.parse().ok(),
                        "PLAYER_TECH_LEVEL" => self.tavern_tier = value.parse().ok(),
                        _ => {}
                    }
                }
                self.refresh_lists();
            }
            LogEvent::FullEntity {
                entity_id,
                card_id,
            } => {
                self.entities
                    .apply_full_entity(*entity_id, card_id.clone());
                if card_id.as_deref() == Some("TB_BaconShopBob") {
                    // CONTROLLER arrives as EntityTag next
                }
                self.refresh_lists();
            }
            LogEvent::ShowEntity {
                entity_id,
                card_id,
            } => {
                if let Some(id) = entity_id {
                    self.entities.apply_full_entity(*id, card_id.clone());
                }
                self.refresh_lists();
            }
        }
    }

    fn refresh_lists(&mut self) {
        self.board = self
            .entities
            .board(self.player_id)
            .iter()
            .map(|e| e.short_label())
            .collect();
        self.shop = self
            .entities
            .shop(self.player_id)
            .iter()
            .map(|e| e.short_label())
            .collect();
        self.hand = self
            .entities
            .hand(self.player_id)
            .iter()
            .map(|e| e.short_label())
            .collect();

        self.refresh_lobby_seats();
        self.maybe_snapshot_opponent_board();

        // Snapshot recruit board while not fighting
        if !self.in_combat {
            if let Some(fp) = self.player_id {
                let snap: Vec<SimMinion> = self
                    .entities
                    .board(Some(fp))
                    .iter()
                    .filter_map(|e| SimMinion::from_entity(e))
                    .collect();
                if !snap.is_empty() {
                    self.pre_combat_us = snap;
                }
            }
        }

        self.maybe_update_combat_odds();
    }

    /// Resolve who we are fighting: prefer `NEXT_OPPONENT_PLAYER_ID`, then name match.
    fn resolve_fight_opponent(&self) -> (Option<u32>, Option<String>) {
        if let Some(pid) = self.next_opponent_player_id {
            if self.player_id == Some(pid) {
                // ignore self
            } else {
                let name = self
                    .known_players
                    .iter()
                    .find(|(id, _)| *id == pid)
                    .map(|(_, n)| n.clone())
                    .or_else(|| {
                        self.opponent_boards
                            .iter()
                            .find(|o| o.player_id == Some(pid))
                            .and_then(|o| {
                                o.name
                                    .clone()
                                    .or_else(|| o.hero_name.clone())
                            })
                    });
                return (Some(pid), name.or_else(|| self.combat_opponent.clone()));
            }
        }
        if let Some(ref n) = self.combat_opponent {
            if let Some((pid, _)) = self
                .known_players
                .iter()
                .find(|(_, name)| name == n || name.starts_with(n) || n.starts_with(name.as_str()))
            {
                return (Some(*pid), Some(n.clone()));
            }
            if let Some(o) = self.opponent_boards.iter().find(|o| {
                o.hero_name.as_deref() == Some(n.as_str())
                    || o.name.as_deref() == Some(n.as_str())
            }) {
                return (o.player_id, Some(n.clone()));
            }
            return (None, Some(n.clone()));
        }
        (None, None)
    }

    /// Freeze the enemy board on first sight this fight (start-of-combat, not DR spawns).
    fn maybe_snapshot_opponent_board(&mut self) {
        if !self.in_combat || self.opponent_board_snapshotted {
            return;
        }
        if self.is_duo && !self.our_fight {
            return;
        }
        let Some(fp) = self.player_id else {
            return;
        };
        let bob = self
            .entities
            .bob_player_id
            .or_else(|| self.entities.infer_bob_player(Some(fp)));
        let Some(bob) = bob else {
            return;
        };
        let them = self.entities.combat_minions(bob);
        if them.is_empty() {
            return;
        }
        let board: Vec<String> = them.iter().map(|e| e.short_label()).collect();
        let (pid, name) = self.resolve_fight_opponent();
        let (place, hero_card_id, hero_name, health) = self.seat_meta(pid, name.as_deref());
        let snap = OpponentSnapshot {
            name: name
                .clone()
                .or_else(|| hero_name.clone())
                .or_else(|| self.combat_opponent.clone()),
            board: board.clone(),
            tavern: self.opponent_tavern,
            last_turn: self.turn,
            place,
            player_id: pid,
            hero_card_id,
            hero_name,
            health,
        };
        self.last_opponent = Some(snap.clone());
        self.upsert_opponent_board(snap);
        self.opponent_board_snapshotted = true;
        self.refresh_lobby_seats();
    }

    fn seat_meta(
        &self,
        pid: Option<u32>,
        name: Option<&str>,
    ) -> (Option<u32>, Option<String>, Option<String>, Option<i32>) {
        let seat = self.opponent_boards.iter().find(|o| {
            (pid.is_some() && o.player_id == pid)
                || (name.is_some()
                    && (o.name.as_deref() == name
                        || o.hero_name.as_deref() == name))
        });
        (
            seat.and_then(|o| o.place),
            seat.and_then(|o| o.hero_card_id.clone()),
            seat.and_then(|o| o.hero_name.clone()),
            seat.and_then(|o| o.health),
        )
    }

    fn maybe_update_combat_odds(&mut self) {
        if !self.in_combat {
            self.combat_odds_locked = false;
            return;
        }
        if self.combat_odds_locked {
            return;
        }
        // In Duo, skip odds while spectating the teammate's fight
        if self.is_duo && !self.our_fight {
            return;
        }
        let Some(fp) = self.player_id else {
            return;
        };
        let bob = self
            .entities
            .bob_player_id
            .or_else(|| self.entities.infer_bob_player(Some(fp)));
        let Some(bob) = bob else {
            return;
        };

        let us_ents = self.entities.combat_minions(fp);
        let them_ents = self.entities.combat_minions(bob);

        let mut us: Vec<SimMinion> = us_ents
            .iter()
            .filter_map(|e| SimMinion::from_entity(e))
            .collect();
        if us.is_empty() {
            us = self
                .entities
                .board(Some(fp))
                .iter()
                .filter_map(|e| SimMinion::from_entity(e))
                .collect();
        }
        if us.is_empty() {
            us = self.pre_combat_us.clone();
        }
        let them: Vec<SimMinion> = them_ents
            .iter()
            .filter_map(|e| SimMinion::from_entity(e))
            .collect();

        // Wait until both sides are known (empty us is allowed only if pre-combat was empty)
        if them.is_empty() {
            return;
        }
        // Prefer waiting for our combat board if we had a recruit board
        if us.is_empty() && !self.pre_combat_us.is_empty() {
            us = self.pre_combat_us.clone();
        }

        let fp_key = format!("{:?}|{:?}", us, them);
        // Debounce: only lock once boards look "full enough" — at least one enemy
        // and (our board or confirmed empty pre-combat).
        if fp_key == self.combat_fp_fingerprint {
            return;
        }
        self.combat_fp_fingerprint = fp_key;

        let mode = if self.is_duo {
            FightMode::Duo
        } else {
            FightMode::Solo
        };
        let seed = (self.turn.unwrap_or(0) as u64)
            .wrapping_mul(1_000_003)
            .wrapping_add(us.len() as u64 * 17 + them.len() as u64);

        let odds = simulate(CombatInput {
            us: &us,
            them: &them,
            our_tavern: self.tavern_tier.unwrap_or(1).max(1),
            their_tavern: self
                .opponent_tavern
                .unwrap_or(self.tavern_tier.unwrap_or(1))
                .max(1),
            damage_cap: self.combat_damage_cap,
            opponent_hp: None,
            mode,
            opponent_name: self.combat_opponent.as_deref(),
            trials: COMBAT_TRIALS,
            seed,
        });

        let (pid, name) = self.resolve_fight_opponent();
        let (place, hero_card_id, hero_name, health) = self.seat_meta(pid, name.as_deref());

        self.last_opponent = Some(OpponentSnapshot {
            name: name.clone().or_else(|| self.combat_opponent.clone()),
            board: odds.their_board.clone(),
            tavern: self.opponent_tavern,
            last_turn: self.turn,
            place,
            player_id: pid,
            hero_card_id: hero_card_id.clone(),
            hero_name: hero_name.clone(),
            health,
        });
        // Prefer the early start-of-combat snapshot; only write board here if we missed it.
        if !self.opponent_board_snapshotted {
            self.upsert_opponent_board(OpponentSnapshot {
                name: name.or_else(|| self.combat_opponent.clone()),
                board: odds.their_board.clone(),
                tavern: self.opponent_tavern,
                last_turn: self.turn,
                place,
                player_id: pid,
                hero_card_id,
                hero_name,
                health,
            });
            self.opponent_board_snapshotted = true;
        }
        self.refresh_lobby_seats();
        self.combat_odds = Some(odds);
        // Lock after we have a real prediction (both sides or empty-board loss)
        if !us.is_empty() || self.pre_combat_us.is_empty() {
            self.combat_odds_locked = true;
        }
    }

    fn upsert_opponent_board(&mut self, snap: OpponentSnapshot) {
        let key_name = snap.name.clone();
        let key_pid = snap.player_id;
        let key_hero = snap.hero_name.clone();
        if let Some(existing) = self.opponent_boards.iter_mut().find(|o| {
            (key_pid.is_some() && o.player_id == key_pid)
                || (key_name.is_some()
                    && (o.name == key_name
                        || o.hero_name == key_name
                        || o.name == key_hero
                        || o.hero_name == key_hero))
        }) {
            if snap.place.is_some() {
                existing.place = snap.place;
            }
            if snap.player_id.is_some() {
                existing.player_id = snap.player_id;
            }
            if snap.hero_card_id.is_some() {
                existing.hero_card_id = snap.hero_card_id.clone();
            }
            if snap.hero_name.is_some() {
                existing.hero_name = snap.hero_name.clone();
            }
            if snap.health.is_some() {
                existing.health = snap.health;
            }
            existing.name = snap.name.or(existing.name.clone());
            if !snap.board.is_empty() {
                existing.board = snap.board;
            }
            existing.tavern = snap.tavern.or(existing.tavern);
            existing.last_turn = snap.last_turn.or(existing.last_turn);
        } else {
            self.opponent_boards.push(snap);
        }
        if self.opponent_boards.len() > 8 {
            let n = self.opponent_boards.len() - 8;
            self.opponent_boards.drain(0..n);
        }
    }

    /// Rebuild lobby seats from `PLAYER_LEADERBOARD_PLACE` + hero entities.
    /// Screen position follows **place** (1 = top of left strip), not a fixed panel.
    pub fn refresh_lobby_seats(&mut self) {
        let mut by_pid: std::collections::HashMap<u32, OpponentSnapshot> =
            std::collections::HashMap::new();

        for (pid, name) in &self.known_players {
            if self.player_id == Some(*pid) {
                continue;
            }
            by_pid.entry(*pid).or_insert_with(|| OpponentSnapshot {
                name: Some(name.clone()),
                player_id: Some(*pid),
                ..Default::default()
            });
        }

        for e in self.entities.entities.values() {
            let Some(pid) = e.player.or(e.controller) else {
                continue;
            };
            if self.player_id == Some(pid) {
                continue;
            }

            if let Some(place) = e.leaderboard_place {
                let slot = by_pid.entry(pid).or_insert_with(|| OpponentSnapshot {
                    player_id: Some(pid),
                    ..Default::default()
                });
                slot.place = Some(place);
                if slot.name.is_none() {
                    if let Some((_, n)) = self.known_players.iter().find(|(id, _)| *id == pid) {
                        slot.name = Some(n.clone());
                    }
                }
            }

            let is_hero = e.card_type.as_deref() == Some("HERO")
                || e.card_id.as_deref().is_some_and(is_hero_card_id);
            if is_hero && e.zone.as_deref() == Some("PLAY") {
                if let Some(card) = e.card_id.as_deref() {
                    if is_hero_card_id(card) {
                        let slot = by_pid.entry(pid).or_insert_with(|| OpponentSnapshot {
                            player_id: Some(pid),
                            ..Default::default()
                        });
                        slot.hero_card_id = Some(card.to_string());
                        slot.hero_name = e
                            .name
                            .clone()
                            .or_else(|| crate::cards::card_name(card));
                        if let Some(h) = e.health {
                            slot.health = Some(h);
                        }
                    }
                }
            }
        }

        for (_pid, mut seat) in by_pid {
            if let Some(existing) = self.opponent_boards.iter_mut().find(|o| {
                o.player_id == seat.player_id
                    || (seat.name.is_some() && o.name == seat.name)
                    || (seat.hero_name.is_some()
                        && (o.hero_name == seat.hero_name || o.name == seat.hero_name))
                    || (seat.name.is_some() && o.hero_name == seat.name)
            }) {
                existing.place = seat.place.or(existing.place);
                existing.player_id = seat.player_id.or(existing.player_id);
                existing.hero_card_id = seat.hero_card_id.or(existing.hero_card_id.clone());
                existing.hero_name = seat.hero_name.or(existing.hero_name.clone());
                existing.health = seat.health.or(existing.health);
                if existing.name.is_none() {
                    existing.name = seat.name.take();
                }
            } else if seat.place.is_some() || seat.hero_card_id.is_some() || seat.name.is_some() {
                self.opponent_boards.push(seat);
            }
        }

        self.opponent_boards.sort_by_key(|o| o.place.unwrap_or(99));
    }

    fn apply_tag(&mut self, entity: &str, tag: &str, value: &str) {
        let is_game = entity == "GameEntity" || entity == "19";
        let is_me = self.is_friendly_entity(entity);

        match tag {
            "BACON_CURRENT_COMBAT_PLAYER_ID" if is_me => {
                if let Ok(v) = value.parse::<u32>() {
                    self.our_fight = self.player_id == Some(v);
                }
            }
            "BACON_IN_COMBAT_PHASE" if is_game => {
                let now = value == "1";
                if now && !self.in_combat {
                    self.combat_odds = None;
                    self.combat_fp_fingerprint.clear();
                    self.combat_odds_locked = false;
                    self.opponent_board_snapshotted = false;
                    // Solo: always our fight. Duo: wait for CURRENT_COMBAT_PLAYER_ID.
                    self.our_fight = !self.is_duo;
                }
                if !now && self.in_combat {
                    // Combat ended — remember who we just fought.
                    if let Some(pid) = self.next_opponent_player_id {
                        self.last_fight_player_id = Some(pid);
                    } else if let (Some(pid), _) = self.resolve_fight_opponent() {
                        self.last_fight_player_id = Some(pid);
                    }
                    self.combat_odds_locked = false;
                    self.our_fight = false;
                    self.opponent_board_snapshotted = false;
                }
                self.in_combat = now;
            }
            "NEXT_OPPONENT_PLAYER_ID"
                if is_me || is_game || self.is_friendly_hero_entity(entity) || self.is_friendly_entity(entity) =>
            {
                if let Ok(v) = value.parse::<u32>() {
                    if v > 0 && self.player_id != Some(v) {
                        self.next_opponent_player_id = Some(v);
                    }
                }
            }
            "BACON_COMBAT_DAMAGE_CAP" if is_game => {
                self.combat_damage_cap = value.parse().ok();
            }
            "BACON_DUO_PLAYER_FIGHTS_FIRST_NEXT_COMBAT"
            | "BACON_DUO_TEAMMATE_PLAYER_ID" => {
                self.is_duo = true;
            }
            "TURN" if is_game || is_me => {
                if let Ok(t) = value.parse() {
                    self.turn = Some(t);
                }
            }
            "RESOURCES" if is_me => {
                self.gold = value.parse().ok();
            }
            "HEALTH" if is_me || self.is_friendly_hero_entity(entity) => {
                self.health = value.parse().ok();
            }
            "ARMOR" if is_me || self.is_friendly_hero_entity(entity) => {
                self.armor = value.parse().ok();
            }
            "DAMAGE" if is_me || self.is_friendly_hero_entity(entity) => {
                self.damage = value.parse().ok();
            }
            "PLAYER_TECH_LEVEL" if is_me || self.is_friendly_hero_entity(entity) => {
                if let Ok(t) = value.parse::<u32>() {
                    if t > 0 {
                        self.tavern_tier = Some(t);
                    }
                }
            }
            "PLAYERLEADERBOARDPLACE" | "PLAYER_LEADERBOARD_PLACE" => {
                // Seat updates live as the lobby reorders — refresh after tag applied to entity store.
                self.refresh_lobby_seats();
            }
            "HERO_ENTITY" if is_me => {
                if let Ok(id) = value.parse::<i32>() {
                    // Always follow current hero entity for HP/armor (combat swaps this).
                    self.hero_entity_id = Some(id);
                    // Lock display hero only once — combat temporarily points HERO_ENTITY at clones.
                    if self.hero_card_id.is_none() {
                        if let Some(e) = self.entities.entities.get(&id) {
                            if let Some(card) = e.card_id.as_deref().filter(|c| is_hero_card_id(c))
                            {
                                self.hero_card_id = Some(card.to_string());
                                self.hero_name = e
                                    .name
                                    .clone()
                                    .or_else(|| crate::cards::card_name(card));
                                self.mode = GameMode::Battlegrounds;
                            }
                        }
                    }
                }
            }
            "ZONE" => {
                if value == "PLAY" {
                    if let Some(card) = entity_card_id(entity) {
                        if is_hero_card_id(card)
                            && self.is_friendly_hero_candidate(entity)
                            && self.hero_card_id.is_none()
                        {
                            // First friendly hero into PLAY = our pick (draft lock).
                            self.hero_card_id = Some(card.to_string());
                            if let Some(name) = entity_name(entity) {
                                self.hero_name = Some(name);
                            } else if let Some(n) = crate::cards::card_name(card) {
                                self.hero_name = Some(n);
                            }
                            if let Some(id) = entity_id(entity) {
                                self.hero_entity_id = Some(id);
                            }
                            if self.health.is_none() {
                                self.health = Some(30);
                            }
                            self.mode = GameMode::Battlegrounds;
                        }
                    }
                }
                if value == "HAND" {
                    if let Some(card) = entity_card_id(entity) {
                        if is_hero_card_id(card) && self.is_friendly_hero_candidate(entity) {
                            if !self.draft_offers.contains(&card.to_string()) {
                                self.draft_offers.push(card.to_string());
                            }
                        }
                    }
                }
            }
            "GAME_TYPE" if is_game => {
                if let Ok(v) = value.parse::<i32>() {
                    self.apply_game_type(v);
                }
            }
            _ => {
                if self.in_combat
                    && !is_me
                    && !is_game
                    && !entity.starts_with('[')
                    && entity.len() >= 3
                    && entity.chars().any(|c| c.is_alphabetic())
                {
                    if tag.starts_with("BACON_") || tag == "SPAWN_TIME_COUNT" || tag == "DAMAGE" {
                        let banned = matches!(
                            entity,
                            "UNKNOWN HUMAN PLAYER"
                                | "GameEntity"
                                | "BaconShop8PlayerEnchant"
                                | "Drag To Buy"
                                | "Refresh"
                                | "Freeze"
                        );
                        if !banned
                            && self.player_name.as_deref() != Some(entity)
                            && (self.known_players.iter().any(|(_, n)| n == entity)
                                || entity.contains('#')
                                || (!entity.contains('_') && !entity.chars().all(|c| c.is_ascii_digit())))
                        {
                            self.combat_opponent = Some(entity.to_string());
                        }
                    }
                }
            }
        }
    }

    fn is_friendly_entity(&self, entity: &str) -> bool {
        if let Some(ref name) = self.player_name {
            if entity == name || entity.starts_with(name) {
                return true;
            }
        }
        if let Some(pid) = self.player_id {
            if entity_player(entity) == Some(pid) {
                return true;
            }
        }
        false
    }

    fn is_friendly_hero_entity(&self, entity: &str) -> bool {
        // Prefer the exact HERO_ENTITY id so combat hero clones don't steal HP/armor.
        if let Some(hid) = self.hero_entity_id {
            return entity_id(entity) == Some(hid) || entity == hid.to_string();
        }
        self.is_friendly_hero_candidate(entity)
            && entity_card_id(entity).is_some_and(is_hero_card_id)
    }

    /// Battlegrounds "shop turn" ≈ ceil(GameEntity.TURN / 2).
    pub fn bg_turn(&self) -> Option<u32> {
        self.turn.map(|t| t.div_ceil(2).max(1))
    }

    fn is_friendly_hero_candidate(&self, entity: &str) -> bool {
        if let Some(pid) = self.player_id {
            return entity_player(entity) == Some(pid);
        }
        if let Some(hid) = self.hero_entity_id {
            if entity_id(entity) == Some(hid) {
                return true;
            }
        }
        // Before ids known: only treat as candidate if it looks like a BG hero card
        entity_card_id(entity).is_some_and(is_hero_card_id)
    }

    /// Stable summary for CLI (no per-event counters — avoids spam).
    pub fn summary_line(&self) -> String {
        let hero = self
            .hero_name
            .as_deref()
            .or(self.hero_card_id.as_deref())
            .unwrap_or("-");
        let hp = match (self.health, self.damage, self.armor) {
            (Some(h), dmg, Some(a)) if a > 0 => {
                let net = h.saturating_sub(dmg.unwrap_or(0));
                format!("{net}+{a}")
            }
            (Some(h), dmg, _) => h.saturating_sub(dmg.unwrap_or(0)).to_string(),
            (None, _, Some(a)) if a > 0 => format!("?+{a}"),
            _ => "-".into(),
        };
        let combat = match (&self.combat_odds, self.in_combat, self.our_fight) {
            (Some(o), true, _) => {
                let vs = o
                    .opponent_name
                    .as_deref()
                    .or(self.combat_opponent.as_deref())
                    .unwrap_or("?");
                format!(" | COMBAT vs {vs}: {}", o.summary_short())
            }
            (None, true, false) if self.is_duo => " | COMBAT (partner)…".into(),
            (None, true, _) => " | COMBAT…".into(),
            // Keep last odds only while still relevant (just left combat) — omit otherwise
            (Some(o), false, _) if self.queue == QueueStatus::InMatch => {
                format!(" | last fight: {}", o.summary_short())
            }
            _ => String::new(),
        };
        let duo = if self.is_duo { " duo" } else { "" };
        format!(
            "mode={:?}{duo} queue={:?} turn={} gold={} hp={} tavern={} hero={} | shop={} board={} hand={}{combat}",
            self.mode,
            self.queue,
            opt_u(self.turn),
            opt_u(self.gold),
            hp,
            opt_u(self.tavern_tier),
            hero,
            fmt_list(&self.shop),
            fmt_list(&self.board),
            fmt_list(&self.hand),
        )
    }
}

fn fmt_list(items: &[String]) -> String {
    if items.is_empty() {
        "[]".into()
    } else {
        format!("[{}]", items.join(", "))
    }
}

fn map_find_game_state(state: &str) -> QueueStatus {
    match state {
        "CLIENT_STARTED" | "BNET_QUEUE_ENTERED" | "BNET_QUEUE_DELAYED" | "BNET_QUEUE_UPDATED" => {
            QueueStatus::Searching
        }
        "SERVER_GAME_CONNECTING" => QueueStatus::Connecting,
        "SERVER_GAME_STARTED" => QueueStatus::InMatch,
        "CLIENT_CANCELED" | "BNET_QUEUE_CANCELED" | "SERVER_GAME_CANCELED" | "INVALID" => {
            QueueStatus::Idle
        }
        _ => QueueStatus::Idle,
    }
}

fn opt_u(v: Option<u32>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "-".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_gallywix_not_lich_king() {
        let mut s = LiveState::default();
        s.apply(&LogEvent::PlayerInfo {
            player_id: 7,
            name: "fr4iser#2425".into(),
        });
        s.apply(&LogEvent::TagChange {
            entity: "fr4iser#2425".into(),
            tag: "HERO_ENTITY".into(),
            value: "114".into(),
        });
        s.apply(&LogEvent::TagChange {
            entity: "[entityName=Trade Prince Gallywix id=114 zone=HAND zonePos=2 cardId=TB_BaconShop_HERO_10 player=7]".into(),
            tag: "ZONE".into(),
            value: "PLAY".into(),
        });
        s.apply(&LogEvent::TagChange {
            entity: "fr4iser#2425".into(),
            tag: "RESOURCES".into(),
            value: "3".into(),
        });
        // Noise from another player's hero
        s.apply(&LogEvent::TagChange {
            entity: "[entityName=The Lich King id=225 zone=SETASIDE zonePos=0 cardId=TB_BaconShop_HERO_22_SKIN_F player=15]".into(),
            tag: "ZONE".into(),
            value: "PLAY".into(),
        });
        assert_eq!(s.hero_card_id.as_deref(), Some("TB_BaconShop_HERO_10"));
        assert_eq!(s.hero_name.as_deref(), Some("Trade Prince Gallywix"));
        assert_eq!(s.gold, Some(3));
    }

    #[test]
    fn duo_game_type_sets_flag() {
        let mut s = LiveState::default();
        s.apply(&LogEvent::GameType {
            value: GAME_TYPE_BATTLEGROUNDS_DUO,
        });
        assert_eq!(s.mode, GameMode::Battlegrounds);
        assert!(s.is_duo);
    }
}

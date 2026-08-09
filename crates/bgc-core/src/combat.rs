//! Battlegrounds combat Monte-Carlo (basic mechanics).
//!
//! Models: left-to-right attacks, random target (taunt priority), divine shield,
//! poisonous, reborn (1 HP), windfury (2 attacks), simple deathrattle summons.
//! Missing: cleave, complex DRs, start-of-combat, hero powers, most auras.
//! Odds are therefore **approximate** — never claim certainty unless every trial agrees.

use crate::cards::deathrattle_summons;
use crate::entities::Entity;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FightMode {
    Solo,
    /// Current fight is still 1v1; duo just labels the lobby format.
    Duo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimMinion {
    pub card_id: String,
    pub name: Option<String>,
    pub atk: i32,
    pub health: i32,
    pub tech_level: u32,
    pub divine_shield: bool,
    pub taunt: bool,
    pub reborn: bool,
    pub poisonous: bool,
    pub windfury: bool,
    pub mega_windfury: bool,
}

impl SimMinion {
    pub fn from_entity(e: &Entity) -> Option<Self> {
        let atk = e.atk.unwrap_or(0);
        let health = e.health.unwrap_or(0);
        if health <= 0 && atk <= 0 {
            return None;
        }
        let card_id = e.card_id.clone().unwrap_or_default();
        let def = crate::cards::card_def(&card_id);
        let name = e
            .name
            .clone()
            .filter(|n| !n.is_empty() && n != "?")
            .or_else(|| def.as_ref().and_then(|d| d.name.clone()));
        Some(Self {
            card_id,
            name,
            atk,
            health: health.max(1),
            tech_level: e
                .tech_level
                .or_else(|| def.as_ref().and_then(|d| d.tech_level))
                .unwrap_or(1)
                .max(1),
            divine_shield: e.divine_shield
                || def.as_ref().is_some_and(|d| d.is_divine_shield()),
            taunt: e.taunt || def.as_ref().is_some_and(|d| d.is_taunt()),
            reborn: e.reborn || def.as_ref().is_some_and(|d| d.is_reborn()),
            poisonous: e.poisonous || def.as_ref().is_some_and(|d| d.is_poisonous()),
            windfury: e.windfury || def.as_ref().is_some_and(|d| d.is_windfury()),
            mega_windfury: e.mega_windfury
                || def
                    .as_ref()
                    .is_some_and(|d| d.has_mechanic("MEGA_WINDFURY")),
        })
    }

    fn attacks_per_turn(&self) -> u8 {
        if self.mega_windfury {
            4
        } else if self.windfury {
            2
        } else {
            1
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatOdds {
    pub mode: FightMode,
    pub trials: u32,
    pub win_pct: f32,
    pub tie_pct: f32,
    pub lose_pct: f32,
    pub damage_min: i32,
    pub damage_max: i32,
    pub damage_avg: f32,
    /// Chance we deal lethal given opponent HP (if known).
    pub lethal_pct: Option<f32>,
    pub our_board: Vec<String>,
    pub their_board: Vec<String>,
    pub opponent_name: Option<String>,
    pub approximate: bool,
    pub note: Option<String>,
}

impl CombatOdds {
    pub fn summary_short(&self) -> String {
        let lethal = match self.lethal_pct {
            Some(p) if p >= 99.5 => " lethal!".to_string(),
            Some(p) if p > 0.5 => format!(" lethal~{p:.0}%"),
            _ => String::new(),
        };
        let approx = if self.approximate { "~" } else { "" };
        let mode = match self.mode {
            FightMode::Solo => "",
            FightMode::Duo => "duo ",
        };
        format!(
            "{mode}{approx}W{:.0}% T{:.0}% L{:.0}% dmg {}-{} (avg {:.1}){lethal}",
            self.win_pct,
            self.tie_pct,
            self.lose_pct,
            self.damage_min,
            self.damage_max,
            self.damage_avg
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CombatInput<'a> {
    pub us: &'a [SimMinion],
    pub them: &'a [SimMinion],
    pub our_tavern: u32,
    pub their_tavern: u32,
    pub damage_cap: Option<i32>,
    pub opponent_hp: Option<i32>,
    pub mode: FightMode,
    pub opponent_name: Option<&'a str>,
    pub trials: u32,
    pub seed: u64,
}

pub fn simulate(input: CombatInput<'_>) -> CombatOdds {
    let trials = input.trials.max(1);
    let mut rng = SmallRng::seed_from_u64(input.seed);
    let mut wins = 0u32;
    let mut ties = 0u32;
    let mut losses = 0u32;
    let mut dmg_sum = 0i64;
    let mut dmg_min = i32::MAX;
    let mut dmg_max = i32::MIN;
    let mut lethals = 0u32;

    for _ in 0..trials {
        let (outcome, damage) = fight_once(
            input.us,
            input.them,
            input.our_tavern,
            input.their_tavern,
            input.damage_cap,
            &mut rng,
        );
        match outcome {
            1 => wins += 1,
            0 => ties += 1,
            _ => losses += 1,
        }
        // Signed damage from our POV: positive = we deal, negative = we take
        dmg_sum += damage as i64;
        dmg_min = dmg_min.min(damage);
        dmg_max = dmg_max.max(damage);
        if let Some(hp) = input.opponent_hp {
            if damage > 0 && damage >= hp {
                lethals += 1;
            }
        }
    }

    let t = trials as f32;
    let approximate = boards_need_approx(input.us) || boards_need_approx(input.them);
    CombatOdds {
        mode: input.mode,
        trials,
        win_pct: wins as f32 / t * 100.0,
        tie_pct: ties as f32 / t * 100.0,
        lose_pct: losses as f32 / t * 100.0,
        damage_min: if dmg_min == i32::MAX { 0 } else { dmg_min },
        damage_max: if dmg_max == i32::MIN { 0 } else { dmg_max },
        damage_avg: dmg_sum as f32 / t,
        lethal_pct: input.opponent_hp.map(|_| lethals as f32 / t * 100.0),
        our_board: input
            .us
            .iter()
            .map(|m| format!("{} {}/{}", m.name.as_deref().unwrap_or(&m.card_id), m.atk, m.health))
            .collect(),
        their_board: input
            .them
            .iter()
            .map(|m| format!("{} {}/{}", m.name.as_deref().unwrap_or(&m.card_id), m.atk, m.health))
            .collect(),
        opponent_name: input.opponent_name.map(|s| s.to_string()),
        approximate,
        note: if approximate {
            Some("basic sim (keywords+simple DR; missing complex effects)".into())
        } else {
            None
        },
    }
}

fn boards_need_approx(board: &[SimMinion]) -> bool {
    // Always approximate until full card DB exists; still useful.
    !board.is_empty()
}

/// Returns (outcome: 1 win / 0 tie / -1 loss, signed_damage from our POV).
fn fight_once(
    us: &[SimMinion],
    them: &[SimMinion],
    our_tavern: u32,
    their_tavern: u32,
    damage_cap: Option<i32>,
    rng: &mut SmallRng,
) -> (i8, i32) {
    let mut a = us.to_vec();
    let mut b = them.to_vec();
    let mut next_a = 0usize;
    let mut next_b = 0usize;

    // First attacker: more minions, else coin flip
    let mut a_turn = if a.len() > b.len() {
        true
    } else if b.len() > a.len() {
        false
    } else {
        rng.gen_bool(0.5)
    };

    let mut safety = 0u32;
    while !a.is_empty() && !b.is_empty() && safety < 500 {
        safety += 1;
        if a_turn {
            if next_a >= a.len() {
                next_a = 0;
            }
            if a.is_empty() {
                break;
            }
            let attacks = a[next_a].attacks_per_turn();
            for _ in 0..attacks {
                if a.is_empty() || b.is_empty() {
                    break;
                }
                if next_a >= a.len() {
                    next_a = 0;
                }
                let target = pick_target(&b, rng);
                resolve_hit(&mut a, next_a, &mut b, target, rng);
                // compact dead
                remove_dead(&mut a, &mut next_a);
                remove_dead(&mut b, &mut 0);
                if next_a > a.len() {
                    next_a = 0;
                }
            }
            if !a.is_empty() {
                next_a = (next_a + 1) % a.len().max(1);
            }
        } else {
            if next_b >= b.len() {
                next_b = 0;
            }
            if b.is_empty() {
                break;
            }
            let attacks = b[next_b].attacks_per_turn();
            for _ in 0..attacks {
                if a.is_empty() || b.is_empty() {
                    break;
                }
                if next_b >= b.len() {
                    next_b = 0;
                }
                let target = pick_target(&a, rng);
                resolve_hit(&mut b, next_b, &mut a, target, rng);
                remove_dead(&mut b, &mut next_b);
                remove_dead(&mut a, &mut 0);
                if next_b > b.len() {
                    next_b = 0;
                }
            }
            if !b.is_empty() {
                next_b = (next_b + 1) % b.len().max(1);
            }
        }
        a_turn = !a_turn;
    }

    let damage = |survivors: &[SimMinion], tavern: u32| -> i32 {
        let stars: u32 = survivors.iter().map(|m| m.tech_level).sum::<u32>() + tavern;
        let mut d = stars as i32;
        if let Some(cap) = damage_cap {
            d = d.min(cap);
        }
        d
    };

    if a.is_empty() && b.is_empty() {
        (0, 0)
    } else if b.is_empty() {
        (1, damage(&a, our_tavern))
    } else if a.is_empty() {
        (-1, -damage(&b, their_tavern))
    } else {
        // safety cap — treat as tie
        (0, 0)
    }
}

fn pick_target(enemies: &[SimMinion], rng: &mut SmallRng) -> usize {
    let taunts: Vec<usize> = enemies
        .iter()
        .enumerate()
        .filter(|(_, m)| m.taunt)
        .map(|(i, _)| i)
        .collect();
    if !taunts.is_empty() {
        taunts[rng.gen_range(0..taunts.len())]
    } else {
        rng.gen_range(0..enemies.len())
    }
}

fn resolve_hit(
    attackers: &mut Vec<SimMinion>,
    ai: usize,
    defenders: &mut Vec<SimMinion>,
    di: usize,
    _rng: &mut SmallRng,
) {
    if ai >= attackers.len() || di >= defenders.len() {
        return;
    }
    let atk_a = attackers[ai].atk;
    let atk_d = defenders[di].atk;
    let pois_a = attackers[ai].poisonous;
    let pois_d = defenders[di].poisonous;

    apply_damage(&mut defenders[di], atk_a, pois_a);
    apply_damage(&mut attackers[ai], atk_d, pois_d);
}

fn apply_damage(m: &mut SimMinion, dmg: i32, poisonous: bool) {
    if dmg <= 0 && !poisonous {
        return;
    }
    if m.divine_shield {
        m.divine_shield = false;
        return;
    }
    if poisonous && dmg > 0 {
        m.health = 0;
    } else {
        m.health -= dmg;
    }
}

fn remove_dead(board: &mut Vec<SimMinion>, next_idx: &mut usize) {
    let mut i = 0;
    while i < board.len() {
        if board[i].health <= 0 {
            let dead = board.remove(i);
            if *next_idx > i {
                *next_idx -= 1;
            } else if *next_idx >= board.len() && !board.is_empty() {
                *next_idx = 0;
            }
            // Reborn
            if dead.reborn {
                let mut revived = dead.clone();
                revived.health = 1;
                revived.reborn = false;
                revived.divine_shield = false;
                board.insert(i, revived);
                i += 1;
                continue;
            }
            // Deathrattle summons (append to end)
            for summon in deathrattle_summons(&dead.card_id) {
                for _ in 0..summon.count {
                    if board.len() >= 7 {
                        break;
                    }
                    board.push(SimMinion {
                        card_id: summon.card_id.clone(),
                        name: summon.name.clone(),
                        atk: summon.atk,
                        health: summon.health,
                        tech_level: 1,
                        divine_shield: false,
                        taunt: false,
                        reborn: false,
                        poisonous: false,
                        windfury: false,
                        mega_windfury: false,
                    });
                }
            }
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(atk: i32, health: i32, tech: u32) -> SimMinion {
        SimMinion {
            card_id: "X".into(),
            name: Some("X".into()),
            atk,
            health,
            tech_level: tech,
            divine_shield: false,
            taunt: false,
            reborn: false,
            poisonous: false,
            windfury: false,
            mega_windfury: false,
        }
    }

    #[test]
    fn stronger_board_usually_wins() {
        let us = vec![m(10, 10, 5), m(10, 10, 5)];
        let them = vec![m(1, 1, 1)];
        let odds = simulate(CombatInput {
            us: &us,
            them: &them,
            our_tavern: 1,
            their_tavern: 1,
            damage_cap: None,
            opponent_hp: Some(5),
            mode: FightMode::Solo,
            opponent_name: None,
            trials: 200,
            seed: 1,
        });
        assert!(odds.win_pct > 90.0, "{odds:?}");
        assert!(odds.damage_min > 0);
        assert!(odds.lethal_pct.unwrap_or(0.0) > 90.0, "{odds:?}");
    }

    #[test]
    fn empty_boards_tie() {
        let odds = simulate(CombatInput {
            us: &[],
            them: &[],
            our_tavern: 1,
            their_tavern: 1,
            damage_cap: None,
            opponent_hp: None,
            mode: FightMode::Solo,
            opponent_name: None,
            trials: 50,
            seed: 2,
        });
        assert!(odds.tie_pct > 99.0);
    }
}

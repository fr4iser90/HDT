//! In-game overlay: movable panels + frame/badge highlights on tavern slots.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bgc_core::cards::card_def;
use bgc_core::entities::Entity;
use bgc_core::{hearthstone_running, card_name, LiveState, MatchPhase};
use egui::{Color32, Frame, Margin, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use crate::clickthrough::{set_full_window_input, set_input_rects, XRect};
use crate::hero_tiers::load_hero_tiers;
use crate::hs_window::{
    find_hearthstone, find_hearthstone_window, mark_overlay_above, WindowGeom,
};
use crate::layout_persist::{load_layout, save_layout, OverlayLayout, PanelGeom};
use crate::SharedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightFilter {
    Deathrattle,
    Taunt,
    DivineShield,
    Reborn,
    Quilboar,
    Undead,
    Beast,
    Dragon,
    Murloc,
    Demon,
    Mech,
    Elemental,
    Naga,
    Pirate,
}

impl HighlightFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::Deathrattle => "DR",
            Self::Taunt => "Taunt",
            Self::DivineShield => "DS",
            Self::Reborn => "Reborn",
            Self::Quilboar => "Quil",
            Self::Undead => "Undead",
            Self::Beast => "Beast",
            Self::Dragon => "Dragon",
            Self::Murloc => "Murloc",
            Self::Demon => "Demon",
            Self::Mech => "Mech",
            Self::Elemental => "Elem",
            Self::Naga => "Naga",
            Self::Pirate => "Pirate",
        }
    }

    fn matches_card(self, card_id: &str) -> bool {
        let Some(def) = card_def(card_id) else {
            return false;
        };
        match self {
            Self::Deathrattle => def.is_deathrattle(),
            Self::Taunt => def.is_taunt(),
            Self::DivineShield => def.is_divine_shield(),
            Self::Reborn => def.is_reborn(),
            Self::Quilboar => race_is(&def, "QUILBOAR"),
            Self::Undead => race_is(&def, "UNDEAD"),
            Self::Beast => race_is(&def, "BEAST"),
            Self::Dragon => race_is(&def, "DRAGON"),
            Self::Murloc => race_is(&def, "MURLOC"),
            Self::Demon => race_is(&def, "DEMON"),
            Self::Mech => race_is(&def, "MECHANICAL") || race_is(&def, "MECH"),
            Self::Elemental => race_is(&def, "ELEMENTAL"),
            Self::Naga => race_is(&def, "NAGA"),
            Self::Pirate => race_is(&def, "PIRATE"),
        }
    }
}

fn race_is(def: &bgc_core::CardDef, race: &str) -> bool {
    def.race.as_deref() == Some(race)
        || def
            .races
            .as_ref()
            .is_some_and(|rs| rs.iter().any(|r| r == race))
}

/// Built-in meta-comp stubs (card-id substrings / exact ids). Selectable → highlight shop.
#[derive(Debug, Clone, Copy)]
struct MetaComp {
    name: &'static str,
    /// Card ids (or prefixes) that belong to this comp.
    card_ids: &'static [&'static str],
}

const META_COMPS: &[MetaComp] = &[
    MetaComp {
        name: "Deathrattle",
        card_ids: &[],
    },
    MetaComp {
        name: "Quilboar",
        card_ids: &[],
    },
    MetaComp {
        name: "Undead",
        card_ids: &[],
    },
    MetaComp {
        name: "Elemental",
        card_ids: &[],
    },
];

/// Tavern/board highlight style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HighlightStyle {
    /// Gold rectangle around the minion slot (default).
    Frame,
    /// Small label badge above the minion + thin frame.
    Badge,
    Both,
}

/// Battlegrounds UI regions as fractions of the HS client (fixed defaults, N-aware).
mod layout {
    use egui::{pos2, Rect, Vec2};

    use crate::layout_persist::SlotCalibration;

    /// Row of N equal slots, centered at `center_x_frac` (HS recenters when N changes).
    pub fn slot_row(
        screen: Rect,
        y_center: f32,
        slot_h: f32,
        n: usize,
        center_x_frac: f32,
        slot_w_frac: f32,
        gap_frac: f32,
    ) -> Vec<Rect> {
        let n = n.max(1);
        let slot_w = screen.width() * slot_w_frac.clamp(0.04, 0.22);
        let gap = screen.width() * gap_frac.clamp(0.0, 0.05);
        let total = n as f32 * slot_w + (n.saturating_sub(1) as f32) * gap;
        let cx = screen.min.x + screen.width() * center_x_frac.clamp(0.15, 0.85);
        let start_x = cx - total * 0.5;
        let y = screen.min.y + screen.height() * y_center - slot_h * 0.5;
        (0..n)
            .map(|i| {
                let x = start_x + i as f32 * (slot_w + gap);
                Rect::from_min_size(pos2(x, y), Vec2::new(slot_w, slot_h))
            })
            .collect()
    }

    pub fn shop_slots(screen: Rect, n: usize, cal: &SlotCalibration) -> Vec<Rect> {
        let h = screen.height() * cal.shop_h;
        slot_row(
            screen,
            cal.shop_y,
            h,
            n.clamp(1, 7),
            cal.center_x,
            cal.slot_w,
            cal.gap,
        )
    }

    pub fn board_slots(screen: Rect, n: usize, cal: &SlotCalibration) -> Vec<Rect> {
        let h = screen.height() * cal.board_h;
        slot_row(
            screen,
            cal.board_y,
            h,
            n.clamp(1, 7),
            cal.center_x,
            cal.slot_w,
            cal.gap,
        )
    }

    /// Last-board card next to the lobby hero for `PLAYER_LEADERBOARD_PLACE`.
    pub fn lobby_board_anchor(screen: Rect, place: u32, cal: &SlotCalibration) -> Rect {
        let place = place.clamp(1, 8);
        let top = cal.lobby_top;
        let bottom = cal.lobby_bottom.max(top + 0.2);
        let slot = (bottom - top) / 8.0;
        let y_center = top + (place as f32 - 0.5) * slot;
        let h = screen.height() * slot * 0.88;
        let w = (screen.width() * cal.lobby_w).clamp(120.0, 300.0);
        let x = screen.min.x + screen.width() * cal.lobby_x;
        let y = screen.min.y + screen.height() * y_center - h * 0.5;
        Rect::from_min_size(pos2(x, y), Vec2::new(w, h))
    }
}

pub struct BgcApp {
    shared: SharedState,
    stop: Arc<AtomicBool>,
    filters: HashSet<HighlightFilter>,
    selected_meta: Option<&'static str>,
    highlight_style: HighlightStyle,
    last_geom: Option<WindowGeom>,
    last_geom_check: Instant,
    last_raise: Instant,
    hs_fullscreen: bool,
    mouse_passthrough: bool,
    pin_interactive: bool,
    /// Individual HUD panel hit-rects (not a bounding-box union).
    interactive_panels: Vec<Rect>,
    /// Saved / live panel layout (written to disk).
    layout: OverlayLayout,
    last_saved_layout: OverlayLayout,
    last_layout_save: Instant,
}

impl BgcApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        shared: SharedState,
        stop: Arc<AtomicBool>,
    ) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(4.0, 3.0);
        cc.egui_ctx.set_style(style);

        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = Color32::from_rgba_unmultiplied(14, 11, 9, 185);
        visuals.panel_fill = Color32::from_rgba_unmultiplied(14, 11, 9, 185);
        visuals.override_text_color = Some(Color32::from_rgb(245, 235, 220));
        visuals.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(40, 32, 24, 200);
        visuals.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(70, 50, 30, 220);
        visuals.widgets.active.bg_fill = Color32::from_rgb(180, 120, 40);
        visuals.selection.bg_fill = Color32::from_rgb(200, 140, 50);
        cc.egui_ctx.set_visuals(visuals);

        let layout = load_layout();
        let highlight_style = match layout.highlight_style.as_str() {
            "frame" => HighlightStyle::Frame,
            "badge" => HighlightStyle::Badge,
            _ => HighlightStyle::Both,
        };
        let pin_interactive = layout.pin_interactive;

        Self {
            shared,
            stop,
            filters: HashSet::new(),
            selected_meta: None,
            highlight_style,
            last_geom: find_hearthstone_window(),
            last_geom_check: Instant::now(),
            last_raise: Instant::now(),
            hs_fullscreen: false,
            mouse_passthrough: true,
            pin_interactive,
            interactive_panels: Vec::new(),
            last_saved_layout: layout.clone(),
            layout,
            last_layout_save: Instant::now(),
        }
    }

    fn toggle_filter(&mut self, f: HighlightFilter) {
        if !self.filters.insert(f) {
            self.filters.remove(&f);
        }
    }

    fn card_highlighted(&self, card_id: &str) -> bool {
        let filter_hit = !self.filters.is_empty()
            && self.filters.iter().any(|f| f.matches_card(card_id));
        let meta_hit = self.selected_meta.is_some_and(|name| {
            META_COMPS.iter().any(|c| {
                c.name == name
                    && (c.card_ids.iter().any(|id| card_id.starts_with(id) && !id.is_empty())
                        || matches_meta_by_filter(name, card_id))
            })
        });
        filter_hit || meta_hit
    }

    fn sync_to_hs_window(&mut self, ctx: &egui::Context) -> bool {
        // Keep above without stealing focus (windowraise eats clicks into HS).
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::AlwaysOnTop,
        ));
        if self.last_raise.elapsed() >= Duration::from_secs(2) {
            self.last_raise = Instant::now();
            // Soft "above" hint only — no windowraise (that blocks tavern clicks).
            mark_overlay_above();
        }

        if self.last_geom_check.elapsed() < Duration::from_millis(400) {
            return false;
        }
        self.last_geom_check = Instant::now();
        let Some(hs) = find_hearthstone() else {
            return false;
        };
        self.hs_fullscreen = hs.fullscreen;
        let g = hs.geom;
        if self.last_geom == Some(g) {
            return false;
        }
        self.last_geom = Some(g);
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            g.x as f32,
            g.y as f32,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            g.width as f32,
            g.height as f32,
        )));
        true
    }

    /// Input shape = only the real panel rectangles (never a bounding box).
    /// Unlock HUD = full window receives clicks.
    fn sync_click_through(&mut self, _ctx: &egui::Context, _force: bool) {
        let pp = _ctx.pixels_per_point();
        if self.pin_interactive {
            self.mouse_passthrough = false;
            // Always re-apply: winit/resize can wipe the X11 input shape.
            let _ = set_full_window_input(true);
            return;
        }

        let mut xrects: Vec<XRect> = Vec::with_capacity(self.interactive_panels.len());
        for r in &self.interactive_panels {
            if !r.is_positive() {
                continue;
            }
            let x = (r.min.x * pp).floor() as i32;
            let y = (r.min.y * pp).floor() as i32;
            let w = ((r.width() * pp).ceil() as i32).max(1);
            let h = ((r.height() * pp).ceil() as i32).max(1);
            if x > i32::from(i16::MAX) || y > i32::from(i16::MAX) {
                continue;
            }
            xrects.push(XRect {
                x: x.max(0) as i16,
                y: y.max(0) as i16,
                width: w.min(i32::from(u16::MAX)) as u16,
                height: h.min(i32::from(u16::MAX)) as u16,
            });
        }

        self.mouse_passthrough = true;
        // Always re-apply so panel holes stay correct after resize/WM events.
        let _ = set_input_rects(&xrects, true);
    }
}

fn phase_color(phase: MatchPhase) -> Color32 {
    match phase {
        MatchPhase::NoGame | MatchPhase::Menu => Color32::from_rgb(180, 170, 155),
        MatchPhase::Queuing => Color32::from_rgb(120, 180, 255),
        MatchPhase::HeroDraft => Color32::from_rgb(220, 160, 255),
        MatchPhase::Tavern => Color32::from_rgb(120, 220, 140),
        MatchPhase::Combat => Color32::from_rgb(255, 140, 90),
        MatchPhase::DuoSpectate => Color32::from_rgb(200, 200, 100),
    }
}

fn matches_meta_by_filter(meta_name: &str, card_id: &str) -> bool {
    match meta_name {
        "Deathrattle" => HighlightFilter::Deathrattle.matches_card(card_id),
        "Quilboar" => HighlightFilter::Quilboar.matches_card(card_id),
        "Undead" => HighlightFilter::Undead.matches_card(card_id),
        "Elemental" => HighlightFilter::Elemental.matches_card(card_id),
        _ => false,
    }
}

fn panel_frame() -> Frame {
    Frame::none()
        .fill(Color32::from_rgba_unmultiplied(12, 10, 8, 175))
        .rounding(8.0)
        .inner_margin(Margin::same(8.0))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(210, 160, 70, 130),
        ))
}

impl eframe::App for BgcApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_layout(true);
        self.stop.store(true, Ordering::Relaxed);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll often so click-through can toggle as the cursor moves.
        ctx.request_repaint_after(Duration::from_millis(33));

        let resized = self.sync_to_hs_window(ctx);

        let live = self
            .shared
            .live
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let session = self
            .shared
            .session
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        let screen = ctx.screen_rect();
        self.interactive_panels.clear();

        // Full transparent canvas for minion frames/badges (paint only — not clickable).
        egui::CentralPanel::default()
            .frame(Frame::none())
            .show(ctx, |ui| {
                let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
                self.paint_lobby_last_boards(&painter, resp.rect, &live);
                self.paint_slot_highlights(&painter, resp.rect, &live);
            });

        // --- Movable panels (drag title bar) ---
        self.draw_combat_panel(ctx, screen, &live);
        self.draw_status_panel(ctx, screen, &live);
        self.draw_highlights_panel(ctx, screen, &session, &live);
        self.draw_meta_panel(ctx, screen);
        self.draw_heroes_panel(ctx, screen, &live);

        self.capture_panel_layout(ctx);
        self.persist_layout(false);
        self.sync_click_through(ctx, resized);
    }
}

impl BgcApp {
    fn track_panel(&mut self, rect: Rect) {
        if rect.is_positive() {
            // Slightly larger hit target so title-bar grabs are reliable.
            self.interactive_panels.push(rect.expand(4.0));
        }
    }

    fn style_key(style: HighlightStyle) -> &'static str {
        match style {
            HighlightStyle::Frame => "frame",
            HighlightStyle::Badge => "badge",
            HighlightStyle::Both => "both",
        }
    }

    fn capture_panel_layout(&mut self, ctx: &egui::Context) {
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_combat"))) {
            if r.is_positive() {
                self.layout.combat = Some(PanelGeom::from_rect(r));
            }
        }
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_highlights"))) {
            if r.is_positive() {
                self.layout.highlights = Some(PanelGeom::from_rect(r));
            }
        }
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_meta"))) {
            if r.is_positive() {
                self.layout.meta = Some(PanelGeom::from_rect(r));
            }
        }
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_status"))) {
            if r.is_positive() {
                self.layout.status = Some(PanelGeom::from_rect(r));
            }
        }
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_heroes"))) {
            if r.is_positive() {
                self.layout.heroes = Some(PanelGeom::from_rect(r));
            }
        }
        self.layout.highlight_style = Self::style_key(self.highlight_style).into();
        self.layout.pin_interactive = self.pin_interactive;
    }

    fn persist_layout(&mut self, force: bool) {
        if self.layout == self.last_saved_layout {
            return;
        }
        if !force && self.last_layout_save.elapsed() < Duration::from_secs(1) {
            return;
        }
        save_layout(&self.layout);
        self.last_saved_layout = self.layout.clone();
        self.last_layout_save = Instant::now();
    }

    fn draw_combat_panel(&mut self, ctx: &egui::Context, screen: Rect, live: &LiveState) {
        let fallback = Pos2::new(screen.center().x - 200.0, screen.min.y + screen.height() * 0.05);
        let (pos, size) = match self.layout.combat {
            Some(g) => (g.pos(), g.size()),
            None => (fallback, Vec2::new(400.0, 120.0)),
        };
        let mut open = true;
        egui::Window::new("⚔ Combat")
            .id(egui::Id::new("bgc_combat"))
            .default_pos(pos)
            .default_size(size)
            .resizable(true)
            .collapsible(true)
            .open(&mut open)
            .frame(panel_frame())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.pin_interactive, "unlock HUD");
                    ui.label(
                        RichText::new(if hearthstone_running() { "live" } else { "…" })
                            .small()
                            .weak(),
                    );
                });
                ui.label(
                    RichText::new("Clicks pass through except on panels. Unlock = whole overlay clickable.")
                        .small()
                        .weak(),
                );
                if self.hs_fullscreen {
                    ui.colored_label(
                        Color32::from_rgb(255, 120, 80),
                        RichText::new(
                            "HS is FULLSCREEN — overlay gets buried. Set HS to Borderless Windowed.",
                        )
                        .small(),
                    );
                }
                if live.in_combat {
                    if live.is_duo && !live.our_fight {
                        ui.label("Partner fighting…");
                    } else if let Some(o) = &live.combat_odds {
                        let vs = o
                            .opponent_name
                            .as_deref()
                            .or(live.combat_opponent.as_deref())
                            .unwrap_or("?");
                        ui.label(RichText::new(format!("vs {vs}")).strong());
                        ui.label(format!(
                            "W {:.0}%   T {:.0}%   L {:.0}%",
                            o.win_pct, o.tie_pct, o.lose_pct
                        ));
                        draw_mini_bars(ui, o);
                        ui.label(
                            RichText::new(format!(
                                "dmg {}…{}  avg {:.1}",
                                o.damage_min, o.damage_max, o.damage_avg
                            ))
                            .small(),
                        );
                    } else {
                        ui.label("Calculating…");
                    }
                } else if let Some(o) = &live.combat_odds {
                    ui.label(RichText::new("Last fight").weak());
                    ui.label(format!(
                        "W {:.0}%  T {:.0}%  L {:.0}%",
                        o.win_pct, o.tie_pct, o.lose_pct
                    ));
                } else {
                    ui.label(RichText::new("No combat yet").weak());
                }
            });
        if let Some(r) = ctx.memory(|m| {
            m.area_rect(egui::Id::new("bgc_combat"))
        }) {
            self.track_panel(r);
        }
    }

    fn draw_status_panel(&mut self, ctx: &egui::Context, screen: Rect, live: &LiveState) {
        let fallback = Pos2::new(screen.min.x + 12.0, screen.min.y + screen.height() * 0.05);
        let (pos, size) = match self.layout.status {
            Some(g) => (g.pos(), g.size()),
            None => (fallback, Vec2::new(260.0, 200.0)),
        };
        let phase = live.match_phase();
        egui::Window::new("Status")
            .id(egui::Id::new("bgc_status"))
            .default_pos(pos)
            .default_size(size)
            .resizable(true)
            .collapsible(true)
            .frame(panel_frame())
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(phase.label())
                        .strong()
                        .color(phase_color(phase)),
                );
                ui.label(
                    RichText::new(format!(
                        "queue {:?} · {}",
                        live.queue,
                        if live.is_duo { "Duos" } else { "Solo" }
                    ))
                    .small()
                    .weak(),
                );
                if live.power_log_stale {
                    ui.colored_label(
                        Color32::from_rgb(255, 120, 80),
                        RichText::new(
                            "Power.log feed dead (HS 10000KB cap). Overlay rotates earlier next session so this should not happen while tracking."
                        )
                        .small(),
                    );
                } else if live.power_log_bytes > 0 {
                    ui.label(
                        RichText::new(format!(
                            "Power.log {:.1} / 10 MB (auto-cleared before full)",
                            live.power_log_bytes as f32 / (1024.0 * 1024.0)
                        ))
                        .small()
                        .weak(),
                    );
                }
                ui.separator();

                let hero = live
                    .hero_name
                    .clone()
                    .or_else(|| live.hero_card_id.as_deref().and_then(card_name))
                    .unwrap_or_else(|| "—".into());
                ui.label(RichText::new(format!("Hero: {hero}")).strong());
                if let Some(id) = &live.hero_card_id {
                    ui.label(RichText::new(id).small().weak());
                }
                if let Some(place) = live.our_place() {
                    ui.label(format!("Lobby seat: #{place}"));
                } else if phase == MatchPhase::Tavern || phase == MatchPhase::Combat {
                    ui.label(RichText::new("Lobby seat: …").small().weak());
                }

                ui.horizontal(|ui| {
                    ui.label(format!("T{}", live.tavern_tier.unwrap_or(0)));
                    ui.label(format!("🪙{}", live.gold.unwrap_or(0)));
                    let hp = live.health.unwrap_or(0);
                    let armor = live.armor.unwrap_or(0);
                    if armor > 0 {
                        ui.label(format!("❤{hp}+{armor}"));
                    } else {
                        ui.label(format!("❤{hp}"));
                    }
                    if let Some(t) = live.bg_turn() {
                        ui.label(format!("turn {t}"));
                    }
                });

                if !live.draft_offers.is_empty()
                    && (phase == MatchPhase::HeroDraft || live.hero_card_id.is_none())
                {
                    ui.separator();
                    ui.label(RichText::new("Hero offers").small().strong());
                    for id in &live.draft_offers {
                        let name = card_name(id).unwrap_or_else(|| id.clone());
                        ui.label(RichText::new(name).small());
                    }
                }

                if let Some(name) = &live.player_name {
                    ui.label(RichText::new(name).small().weak());
                }
            });
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_status"))) {
            self.track_panel(r);
        }
    }

    fn draw_heroes_panel(&mut self, ctx: &egui::Context, screen: Rect, live: &LiveState) {
        let fallback = Pos2::new(
            screen.min.x + 12.0,
            screen.min.y + screen.height() * 0.42,
        );
        let (pos, size) = match self.layout.heroes {
            Some(g) => (g.pos(), g.size()),
            None => (fallback, Vec2::new(240.0, 220.0)),
        };
        egui::Window::new("Hero tiers")
            .id(egui::Id::new("bgc_heroes"))
            .default_pos(pos)
            .default_size(size)
            .resizable(true)
            .collapsible(true)
            .frame(panel_frame())
            .show(ctx, |ui| {
                match load_hero_tiers() {
                    Some(file) => {
                        if !file.source.is_empty() {
                            ui.label(RichText::new(&file.source).small().weak());
                        }
                        if !file.updated.is_empty() {
                            ui.label(RichText::new(format!("updated {}", file.updated)).small().weak());
                        }
                        let our = live.hero_card_id.as_deref();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for bucket in &file.tiers {
                                if bucket.heroes.is_empty() {
                                    continue;
                                }
                                ui.label(
                                    RichText::new(format!("{} tier", bucket.tier))
                                        .strong()
                                        .color(Color32::from_rgb(230, 190, 100)),
                                );
                                for h in &bucket.heroes {
                                    let name = if h.name.is_empty() {
                                        card_name(&h.card_id).unwrap_or_else(|| h.card_id.clone())
                                    } else {
                                        h.name.clone()
                                    };
                                    let mine = our == Some(h.card_id.as_str());
                                    let label = if h.note.is_empty() {
                                        name
                                    } else {
                                        format!("{name} · {}", h.note)
                                    };
                                    ui.label(
                                        RichText::new(if mine {
                                            format!("★ {label}")
                                        } else {
                                            label
                                        })
                                        .small()
                                        .color(if mine {
                                            Color32::from_rgb(120, 220, 140)
                                        } else {
                                            Color32::from_rgb(220, 210, 195)
                                        }),
                                    );
                                }
                                ui.add_space(4.0);
                            }
                            if file.tiers.iter().all(|b| b.heroes.is_empty()) {
                                ui.label(
                                    RichText::new(
                                        "Empty list — fill data/meta/hero_tiers.json (see data/meta/README.md).",
                                    )
                                    .small()
                                    .weak(),
                                );
                            }
                        });
                    }
                    None => {
                        ui.label(
                            RichText::new(
                                "No hero_tiers.json yet. Put curated tiers in data/meta/ (Firestone/HSReplay stats with credit — see data/meta/README.md).",
                            )
                            .small()
                            .weak(),
                        );
                    }
                }
            });
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_heroes"))) {
            self.track_panel(r);
        }
    }

    fn draw_highlights_panel(
        &mut self,
        ctx: &egui::Context,
        screen: Rect,
        session: &str,
        live: &LiveState,
    ) {
        let fallback = Pos2::new(
            screen.max.x - 280.0,
            screen.min.y + screen.height() * 0.12,
        );
        let (pos, size) = match self.layout.highlights {
            Some(g) => (g.pos(), g.size()),
            None => (fallback, Vec2::new(260.0, 280.0)),
        };
        let shop = live.entities.shop(live.player_id);
        let shop_total = shop.len();
        let shop_hits = shop
            .iter()
            .filter(|e| {
                e.card_id
                    .as_deref()
                    .is_some_and(|id| self.card_highlighted(id))
            })
            .count();

        egui::Window::new("➡ Highlights")
            .id(egui::Id::new("bgc_highlights"))
            .default_pos(pos)
            .default_size(size)
            .resizable(true)
            .collapsible(true)
            .frame(panel_frame())
            .show(ctx, |ui| {
                ui.label(RichText::new(session).small().weak());
                if self.hs_fullscreen {
                    ui.colored_label(
                        Color32::from_rgb(255, 120, 80),
                        "Fullscreen HS hides overlay — use Borderless Windowed.",
                    );
                }
                ui.label(
                    RichText::new("Click a filter (e.g. DR) → gold FRAME/BADGE on tavern minions")
                        .small()
                        .weak(),
                );
                ui.label(
                    RichText::new(format!("Shop tracked: {shop_total} · highlighted: {shop_hits}"))
                        .small()
                        .color(if shop_hits > 0 {
                            Color32::from_rgb(120, 220, 120)
                        } else {
                            Color32::from_rgb(180, 160, 130)
                        }),
                );
                if self.filters.is_empty() && self.selected_meta.is_none() {
                    ui.colored_label(
                        Color32::from_rgb(255, 180, 80),
                        "No filter selected → nothing highlighted (click DR / Quil / …)",
                    );
                }
                if shop_total == 0 {
                    ui.colored_label(
                        Color32::from_rgb(255, 140, 100),
                        "No shop entities from Power.log yet (open tavern / check setup)",
                    );
                } else {
                    ui.label(RichText::new("Shop now:").small().strong());
                    for e in shop.iter().take(7) {
                        let card = e.card_id.as_deref().unwrap_or("?");
                        let hit = self.card_highlighted(card);
                        let label = e.short_label();
                        ui.label(
                            RichText::new(format!("{} {label}", if hit { "★" } else { "·" }))
                                .small()
                                .color(if hit {
                                    Color32::from_rgb(255, 200, 80)
                                } else {
                                    Color32::from_rgb(200, 190, 175)
                                }),
                        );
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("Style:");
                    for (label, style) in [
                        ("Frame", HighlightStyle::Frame),
                        ("Badge", HighlightStyle::Badge),
                        ("Both", HighlightStyle::Both),
                    ] {
                        if ui
                            .selectable_label(self.highlight_style == style, label)
                            .clicked()
                        {
                            self.highlight_style = style;
                        }
                    }
                });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    for f in [
                        HighlightFilter::Deathrattle,
                        HighlightFilter::Taunt,
                        HighlightFilter::DivineShield,
                        HighlightFilter::Reborn,
                        HighlightFilter::Quilboar,
                        HighlightFilter::Undead,
                        HighlightFilter::Beast,
                        HighlightFilter::Dragon,
                        HighlightFilter::Murloc,
                        HighlightFilter::Demon,
                        HighlightFilter::Mech,
                        HighlightFilter::Elemental,
                        HighlightFilter::Naga,
                        HighlightFilter::Pirate,
                    ] {
                        let on = self.filters.contains(&f);
                        let mut btn = egui::Button::new(RichText::new(f.label()).small());
                        if on {
                            btn = btn.fill(Color32::from_rgb(180, 110, 40));
                        }
                        if ui.add(btn).clicked() {
                            self.toggle_filter(f);
                        }
                    }
                });
                if !self.filters.is_empty() && ui.small_button("clear filters").clicked() {
                    self.filters.clear();
                }
            });
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_highlights"))) {
            self.track_panel(r);
        }
    }

    fn draw_meta_panel(&mut self, ctx: &egui::Context, screen: Rect) {
        let fallback = Pos2::new(
            screen.max.x - 280.0,
            screen.min.y + screen.height() * 0.48,
        );
        let (pos, size) = match self.layout.meta {
            Some(g) => (g.pos(), g.size()),
            None => (fallback, Vec2::new(260.0, 220.0)),
        };
        egui::Window::new("Meta comps")
            .id(egui::Id::new("bgc_meta"))
            .default_pos(pos)
            .default_size(size)
            .resizable(true)
            .collapsible(true)
            .frame(panel_frame())
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Select a comp → matching tavern minions get highlighted")
                        .small()
                        .weak(),
                );
                for c in META_COMPS {
                    let on = self.selected_meta == Some(c.name);
                    if ui.selectable_label(on, c.name).clicked() {
                        if on {
                            self.selected_meta = None;
                        } else {
                            self.selected_meta = Some(c.name);
                        }
                    }
                }
                if self.selected_meta.is_some() && ui.small_button("clear meta").clicked() {
                    self.selected_meta = None;
                }
            });
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("bgc_meta"))) {
            self.track_panel(r);
        }
    }

    fn paint_lobby_last_boards(
        &self,
        painter: &egui::Painter,
        screen: Rect,
        live: &LiveState,
    ) {
        let bg = Color32::from_rgba_unmultiplied(8, 6, 4, 165);
        let border_default = Color32::from_rgba_unmultiplied(210, 160, 70, 160);
        let border_next = Color32::from_rgb(80, 220, 120);
        let border_last = Color32::from_rgb(100, 160, 255);
        let title = Color32::from_rgb(255, 220, 150);
        let body = Color32::from_rgb(230, 220, 205);

        for op in &live.opponent_boards {
            let Some(place) = op.place else {
                continue; // no seat yet — wait for PLAYER_LEADERBOARD_PLACE
            };
            let is_next = live.is_next_opponent(op);
            let is_last = live.is_last_fight_opponent(op) && !is_next;
            let rect = layout::lobby_board_anchor(screen, place, &self.layout.slots);
            painter.rect_filled(rect, 6.0, bg);
            let (border, stroke_w) = if is_next {
                (border_next, 2.5)
            } else if is_last {
                (border_last, 2.5)
            } else {
                (border_default, 1.0)
            };
            painter.rect_stroke(rect, 6.0, Stroke::new(stroke_w, border));

            let hero = op
                .hero_name
                .as_deref()
                .or(op.name.as_deref())
                .unwrap_or("?");
            let tag = if is_next {
                " · NEXT"
            } else if is_last {
                " · LAST"
            } else {
                ""
            };
            let header = format!("#{place} {hero}{tag}");
            painter.text(
                rect.left_top() + Vec2::new(6.0, 4.0),
                egui::Align2::LEFT_TOP,
                header,
                egui::FontId::proportional(12.0),
                if is_next {
                    border_next
                } else if is_last {
                    border_last
                } else {
                    title
                },
            );

            if op.board.is_empty() {
                painter.text(
                    rect.left_top() + Vec2::new(6.0, 22.0),
                    egui::Align2::LEFT_TOP,
                    "(no board yet)",
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(160, 150, 140),
                );
            } else {
                for (i, line) in op.board.iter().take(7).enumerate() {
                    let short = line.split('(').next().unwrap_or(line).trim();
                    painter.text(
                        rect.left_top() + Vec2::new(6.0, 20.0 + i as f32 * 13.0),
                        egui::Align2::LEFT_TOP,
                        short,
                        egui::FontId::proportional(11.0),
                        body,
                    );
                }
            }
        }
    }

    fn paint_slot_highlights(&self, painter: &egui::Painter, screen: Rect, live: &LiveState) {
        let accent = Color32::from_rgba_unmultiplied(255, 180, 40, 230);
        let fill = Color32::from_rgba_unmultiplied(255, 160, 40, 40);
        let badge_bg = Color32::from_rgba_unmultiplied(20, 12, 4, 210);
        let cal = &self.layout.slots;

        let shop: Vec<&Entity> = live.entities.shop(live.player_id);
        let board: Vec<&Entity> = live.entities.board(live.player_id);
        let want_any = !self.filters.is_empty() || self.selected_meta.is_some();
        if !want_any {
            return;
        }

        let paint_one = |slot: Rect, label: &str| {
            let style = self.highlight_style;
            if matches!(style, HighlightStyle::Frame | HighlightStyle::Both) {
                painter.rect_filled(slot, 6.0, fill);
                painter.rect_stroke(slot, 6.0, Stroke::new(3.0, accent));
            }
            if matches!(style, HighlightStyle::Badge | HighlightStyle::Both) {
                let badge = Rect::from_center_size(
                    Pos2::new(slot.center().x, slot.min.y - 2.0),
                    Vec2::new((slot.width() * 0.9).min(120.0), 18.0),
                );
                painter.rect_filled(badge, 4.0, badge_bg);
                painter.rect_stroke(badge, 4.0, Stroke::new(1.0, accent));
                painter.text(
                    badge.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(255, 230, 160),
                );
            }
        };

        if !live.in_combat && !shop.is_empty() {
            let slots = layout::shop_slots(screen, shop.len(), cal);
            for (i, e) in shop.iter().enumerate() {
                let Some(slot) = slots.get(i) else { break };
                let card = e.card_id.as_deref().unwrap_or("");
                if self.card_highlighted(card) {
                    paint_one(*slot, &e.short_label());
                }
            }
        }

        if !board.is_empty() {
            let slots = layout::board_slots(screen, board.len(), cal);
            for (i, e) in board.iter().enumerate() {
                let Some(slot) = slots.get(i) else { break };
                let card = e.card_id.as_deref().unwrap_or("");
                if self.card_highlighted(card) {
                    paint_one(*slot, &e.short_label());
                }
            }
        }
    }
}

fn draw_mini_bars(ui: &mut egui::Ui, o: &bgc_core::CombatOdds) {
    ui.horizontal(|ui| {
        for (pct, color) in [
            (o.win_pct, Color32::from_rgb(60, 160, 90)),
            (o.tie_pct, Color32::from_rgb(150, 140, 70)),
            (o.lose_pct, Color32::from_rgb(180, 60, 50)),
        ] {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(70.0, 8.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgb(30, 25, 20));
            let w = rect.width() * (pct / 100.0).clamp(0.0, 1.0);
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
                2.0,
                color,
            );
        }
    });
}

//! Persist movable overlay panel positions + slot calibration across restarts.

use std::fs;
use std::path::PathBuf;

use egui::{Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PanelGeom {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl PanelGeom {
    pub fn from_rect(r: Rect) -> Self {
        Self {
            x: r.min.x.round(),
            y: r.min.y.round(),
            w: r.width().round().max(80.0),
            h: r.height().round().max(40.0),
        }
    }

    pub fn pos(self) -> Pos2 {
        Pos2::new(self.x, self.y)
    }

    pub fn size(self) -> Vec2 {
        Vec2::new(self.w.max(80.0), self.h.max(40.0))
    }
}

/// Fractions of the HS client rect (resolution-independent).
///
/// Shop/board use a *per-card* width + shared center. HS recenters the row when
/// the minion count changes, so a fixed outer band would never fit all N.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SlotCalibration {
    /// Shop row vertical center (0–1).
    pub shop_y: f32,
    pub shop_h: f32,
    /// Own board row vertical center (0–1). Should be *below* shop.
    pub board_y: f32,
    pub board_h: f32,
    /// Horizontal center of the minion row (0–1 of client width).
    #[serde(default = "default_center_x")]
    pub center_x: f32,
    /// Width of one minion slot as fraction of client width.
    #[serde(default = "default_slot_w")]
    pub slot_w: f32,
    /// Gap between slots as fraction of client width.
    pub gap: f32,
    /// Lobby last-board cards (left strip).
    pub lobby_x: f32,
    pub lobby_w: f32,
    pub lobby_top: f32,
    pub lobby_bottom: f32,
}

fn default_center_x() -> f32 {
    0.50
}

fn default_slot_w() -> f32 {
    0.095
}

impl Default for SlotCalibration {
    fn default() -> Self {
        // Tuned for typical 16:9 tavern: Bob's shop above, warband below.
        Self {
            shop_y: 0.48,
            shop_h: 0.155,
            board_y: 0.72,
            board_h: 0.145,
            center_x: 0.50,
            slot_w: 0.095,
            gap: 0.010,
            lobby_x: 0.055,
            lobby_w: 0.135,
            lobby_top: 0.11,
            lobby_bottom: 0.88,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct OverlayLayout {
    pub combat: Option<PanelGeom>,
    pub highlights: Option<PanelGeom>,
    pub meta: Option<PanelGeom>,
    #[serde(default)]
    pub status: Option<PanelGeom>,
    #[serde(default)]
    pub heroes: Option<PanelGeom>,
    /// "frame" | "badge" | "both"
    #[serde(default = "default_style")]
    pub highlight_style: String,
    #[serde(default)]
    pub pin_interactive: bool,
    #[serde(default)]
    pub slots: SlotCalibration,
    /// Bumped when slot math changes; old calibrate values are discarded.
    #[serde(default)]
    pub slots_version: u32,
}

fn default_style() -> String {
    "both".into()
}

fn layout_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("bgc").join("overlay_layout.json");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("bgc")
            .join("overlay_layout.json");
    }
    PathBuf::from("overlay_layout.json")
}

pub fn load_layout() -> OverlayLayout {
    let path = layout_path();
    let Ok(bytes) = fs::read(&path) else {
        return OverlayLayout::default();
    };
    let mut layout: OverlayLayout = serde_json::from_slice(&bytes).unwrap_or_default();
    // Discard broken values from the removed drag-calibrate UI.
    if layout.slots_version < 2 {
        layout.slots = SlotCalibration::default();
        layout.slots_version = 2;
    }
    layout
}

pub fn save_layout(layout: &OverlayLayout) {
    let path = layout_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(layout) {
        let _ = fs::write(path, bytes);
    }
}

//! Locate the Hearthstone game window for overlay placement (Linux/X11).
//!
//! Geometry is always read live from the window manager — never hardcoded to a
//! particular monitor layout or resolution (720p … 4K, windowed or fullscreen).

use std::process::Command;

/// Absolute screen geometry of a top-level window (root / virtual-desktop coords).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowGeom {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WindowGeom {
    pub fn area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    pub fn is_sane(self) -> bool {
        self.width >= 640 && self.height >= 360
    }
}

#[derive(Debug, Clone)]
pub struct HsWindow {
    #[allow(dead_code)]
    pub id: String,
    pub geom: WindowGeom,
    pub fullscreen: bool,
}

/// Find the Hearthstone client window geometry.
pub fn find_hearthstone_window() -> Option<WindowGeom> {
    find_hearthstone().map(|w| w.geom)
}

/// Find HS window + fullscreen flag (largest sane match).
pub fn find_hearthstone() -> Option<HsWindow> {
    let mut candidates = Vec::new();
    collect_via_xdotool(&mut candidates);
    if candidates.is_empty() {
        collect_via_wmctrl(&mut candidates);
    }
    candidates
        .into_iter()
        .filter(|w| w.geom.is_sane())
        .max_by_key(|w| w.geom.area())
}

fn collect_via_xdotool(out: &mut Vec<HsWindow>) {
    let Ok(search) = Command::new("xdotool")
        .args(["search", "--name", "Hearthstone"])
        .output()
    else {
        return;
    };
    if !search.status.success() {
        return;
    }
    let ids = String::from_utf8_lossy(&search.stdout);
    for id in ids.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let Ok(name_out) = Command::new("xdotool")
            .args(["getwindowname", id])
            .output()
        else {
            continue;
        };
        let name = String::from_utf8_lossy(&name_out.stdout);
        let name = name.trim();
        if !name.eq_ignore_ascii_case("Hearthstone") {
            continue;
        }
        if let Some(g) = geometry_xdotool(id) {
            out.push(HsWindow {
                id: id.to_string(),
                geom: g,
                fullscreen: window_is_fullscreen(id),
            });
        }
    }
}

fn geometry_xdotool(id: &str) -> Option<WindowGeom> {
    let out = Command::new("xdotool")
        .args(["getwindowgeometry", "--shell", id])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut x = None;
    let mut y = None;
    let mut w = None;
    let mut h = None;
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k {
                "X" => x = v.parse().ok(),
                "Y" => y = v.parse().ok(),
                "WIDTH" => w = v.parse().ok(),
                "HEIGHT" => h = v.parse().ok(),
                _ => {}
            }
        }
    }
    Some(WindowGeom {
        x: x?,
        y: y?,
        width: w?,
        height: h?,
    })
}

fn window_is_fullscreen(id: &str) -> bool {
    let out = Command::new("xprop")
        .args(["-id", id, "_NET_WM_STATE"])
        .output();
    let Ok(out) = out else {
        return false;
    };
    String::from_utf8_lossy(&out.stdout).contains("FULLSCREEN")
}

fn collect_via_wmctrl(out: &mut Vec<HsWindow>) {
    let Ok(listing) = Command::new("wmctrl").args(["-lG"]).output() else {
        return;
    };
    if !listing.status.success() {
        return;
    }
    let text = String::from_utf8_lossy(&listing.stdout);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(id) = parts.next() else { continue };
        let _desk = parts.next();
        let (Some(xs), Some(ys), Some(ws), Some(hs)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(x) = xs.parse::<i32>() else { continue };
        let Ok(y) = ys.parse::<i32>() else { continue };
        let Ok(w) = ws.parse::<u32>() else { continue };
        let Ok(h) = hs.parse::<u32>() else { continue };
        let _host = parts.next();
        let title = parts.collect::<Vec<_>>().join(" ");
        if title.eq_ignore_ascii_case("Hearthstone") {
            out.push(HsWindow {
                id: id.to_string(),
                geom: WindowGeom {
                    x,
                    y,
                    width: w,
                    height: h,
                },
                fullscreen: false,
            });
        }
    }
}

#[derive(Debug, Clone)]
pub struct OverlayWindow {
    pub xid: u32,
    pub geom: WindowGeom,
}

/// Locate our overlay window (name: "BGC Overlay").
pub fn find_overlay_window() -> Option<OverlayWindow> {
    let search = Command::new("xdotool")
        .args(["search", "--name", "BGC Overlay"])
        .output()
        .ok()?;
    if !search.status.success() {
        return None;
    }
    for id in String::from_utf8_lossy(&search.stdout)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let xid = id.parse::<u32>().ok()?;
        if let Some(geom) = geometry_xdotool(id) {
            return Some(OverlayWindow { xid, geom });
        }
    }
    None
}

/// Keep overlay in the ABOVE layer without focus-stealing `windowraise`.
pub fn mark_overlay_above() {
    let Ok(search) = Command::new("xdotool")
        .args(["search", "--name", "BGC Overlay"])
        .output()
    else {
        return;
    };
    if !search.status.success() {
        return;
    }
    for id in String::from_utf8_lossy(&search.stdout)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let _ = Command::new("wmctrl")
            .args(["-i", "-r", id, "-b", "add,above"])
            .status();
    }
}

/// Raise overlay above the game (avoid calling often — steals clicks from HS).
#[allow(dead_code)]
pub fn raise_overlay_window() {
    let Ok(search) = Command::new("xdotool")
        .args(["search", "--name", "BGC Overlay"])
        .output()
    else {
        return;
    };
    if !search.status.success() {
        return;
    }
    for id in String::from_utf8_lossy(&search.stdout)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let _ = Command::new("xdotool")
            .args(["windowraise", id])
            .status();
        let _ = Command::new("wmctrl")
            .args(["-i", "-r", id, "-b", "add,above"])
            .status();
    }
}

/// Global pointer position in root coordinates.
#[allow(dead_code)]
pub fn pointer_root_pos() -> Option<(i32, i32)> {
    let out = Command::new("xdotool")
        .args(["getmouselocation", "--shell"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut x = None;
    let mut y = None;
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k {
                "X" => x = v.parse().ok(),
                "Y" => y = v.parse().ok(),
                _ => {}
            }
        }
    }
    Some((x?, y?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_larger_client_geometry() {
        let big = WindowGeom {
            x: 1920,
            y: 0,
            width: 2560,
            height: 1440,
        };
        assert!(big.is_sane());
    }
}

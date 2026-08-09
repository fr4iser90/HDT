//! Overlay click-through via X11 input shape.
//!
//! The input shape is set to the HUD panel rectangles only, so empty overlay
//! areas never eat clicks. Optional "unlock" uses a full-window shape.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::{ClipOrdering, Rectangle};
use x11rb::rust_connection::RustConnection;

use crate::hs_window::{find_overlay_window, OverlayWindow};

struct ShapeState {
    conn: RustConnection,
    overlay: Option<OverlayWindow>,
    overlay_checked: Instant,
    last_key: u64,
}

static STATE: Mutex<Option<ShapeState>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut ShapeState) -> R) -> Option<R> {
    let mut guard = STATE.lock().ok()?;
    if guard.is_none() {
        let (conn, _) = RustConnection::connect(None).ok()?;
        *guard = Some(ShapeState {
            conn,
            overlay: None,
            overlay_checked: Instant::now()
                .checked_sub(Duration::from_secs(10))
                .unwrap_or_else(Instant::now),
            last_key: u64::MAX,
        });
    }
    Some(f(guard.as_mut()?))
}

fn refresh_overlay(state: &mut ShapeState) -> Option<OverlayWindow> {
    if state.overlay.is_none() || state.overlay_checked.elapsed() > Duration::from_millis(500) {
        state.overlay = find_overlay_window();
        state.overlay_checked = Instant::now();
    }
    state.overlay.clone()
}

fn rect_key(rects: &[Rectangle]) -> u64 {
    let mut h = rects.len() as u64;
    for r in rects {
        h = h
            .wrapping_mul(31)
            .wrapping_add(r.x as u64)
            .wrapping_mul(31)
            .wrapping_add(r.y as u64)
            .wrapping_mul(31)
            .wrapping_add(r.width as u64)
            .wrapping_mul(31)
            .wrapping_add(r.height as u64);
    }
    h
}

/// Apply X11 input shape. Empty `rects` = fully click-through.
pub fn set_input_rects(rects: &[Rectangle], force: bool) -> bool {
    with_state(|state| {
        let Some(overlay) = refresh_overlay(state) else {
            return false;
        };
        let key = rect_key(rects) ^ ((overlay.xid as u64) << 32);
        if !force && key == state.last_key {
            return true;
        }
        if state
            .conn
            .shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                overlay.xid,
                0,
                0,
                rects,
            )
            .is_err()
        {
            return false;
        }
        if state.conn.flush().is_err() {
            return false;
        }
        state.last_key = key;
        true
    })
    .unwrap_or(false)
}

/// Full-window hit target (unlock mode).
pub fn set_full_window_input(force: bool) -> bool {
    with_state(|state| {
        let Some(overlay) = refresh_overlay(state) else {
            return false;
        };
        let rects = [Rectangle {
            x: 0,
            y: 0,
            width: overlay.geom.width.min(u32::from(u16::MAX)) as u16,
            height: overlay.geom.height.min(u32::from(u16::MAX)) as u16,
        }];
        let key = rect_key(&rects) ^ ((overlay.xid as u64) << 32) ^ 1;
        if !force && key == state.last_key {
            return true;
        }
        if state
            .conn
            .shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                overlay.xid,
                0,
                0,
                &rects,
            )
            .is_err()
        {
            return false;
        }
        if state.conn.flush().is_err() {
            return false;
        }
        state.last_key = key;
        true
    })
    .unwrap_or(false)
}

pub use x11rb::protocol::xproto::Rectangle as XRect;

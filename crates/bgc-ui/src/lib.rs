//! Battlegrounds Companion UI — in-game overlay (tracks HS window).

mod app;
mod clickthrough;
mod hero_tiers;
mod hs_window;
mod layout_persist;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Result;
use bgc_core::{
    discover_hs_paths, ensure_card_data_loaded, ensure_log_config, watch_power_log, LiveState,
    WatchOptions, WatchSink,
};

pub use app::{BgcApp, HighlightFilter};
pub use hs_window::{find_hearthstone_window, WindowGeom};

/// Fix display + native libs so the overlay can open on NixOS.
///
/// Prefer `steam-run` (real FHS + X11 libs). LD_LIBRARY_PATH surgery alone is
/// fragile on NixOS (missing libXi in nix-ld, envfs, cargo deps paths, …).
fn prefer_display_backend() {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("BGC_UI_WAYLAND").is_none() {
            // SAFETY: single-threaded startup, before any other threads / UI init.
            unsafe {
                std::env::remove_var("WAYLAND_DISPLAY");
                std::env::remove_var("WAYLAND_SOCKET");
            }
        }
        maybe_reexec_via_steam_run();
        ensure_clean_library_path();
    }
}

/// On NixOS, re-exec under `steam-run` so X11/GL libs resolve like a normal FHS app.
#[cfg(target_os = "linux")]
fn maybe_reexec_via_steam_run() {
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    if std::env::var_os("BGC_INSIDE_STEAM_RUN").is_some() {
        return;
    }
    if std::env::var_os("BGC_UI_NO_STEAM_RUN").is_some() {
        return;
    }
    // Only auto-wrap on NixOS (or when steam-run clearly exists).
    if !Path::new("/nix/store").exists() {
        return;
    }
    let steam_run = ["steam-run", "/run/current-system/sw/bin/steam-run"]
        .into_iter()
        .find(|p| which_exists(p));
    let Some(steam_run) = steam_run else {
        return;
    };

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };

    eprintln!("bgc-ui: NixOS detected — re-exec via steam-run (FHS/X11)");
    let mut cmd = Command::new(steam_run);
    cmd.arg(&exe);
    cmd.args(std::env::args_os().skip(1));
    cmd.env("BGC_INSIDE_STEAM_RUN", "1");
    // steam-run already provides libs — skip LD rewriting in the child.
    cmd.env("BGC_UI_LD_CLEAN", "1");
    if std::env::var_os("BGC_UI_WAYLAND").is_none() {
        cmd.env_remove("WAYLAND_DISPLAY");
        cmd.env_remove("WAYLAND_SOCKET");
    }
    let err = cmd.exec();
    eprintln!("bgc-ui: steam-run re-exec failed: {err} (falling back to LD_LIBRARY_PATH fix)");
}

#[cfg(target_os = "linux")]
fn which_exists(bin: &str) -> bool {
    if bin.contains('/') {
        return std::path::Path::new(bin).is_file();
    }
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p).any(|dir| dir.join(bin).is_file())
        })
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn is_poison_ld_entry(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    l.contains("appimage-run") || l.contains("/appimage/")
}

/// Shared libs winit/x11-dl needs to `dlopen`.
#[cfg(target_os = "linux")]
const X11_LIBS: &[&str] = &[
    "libX11.so.6",
    "libXcursor.so.1",
    "libX11-xcb.so.1",
    "libXi.so.6",
];

/// FHS / envfs paths that often symlink into the nix store.
#[cfg(target_os = "linux")]
const X11_FHS_FILES: &[&str] = &[
    "/usr/lib/libX11.so.6",
    "/usr/lib/libXcursor.so.1",
    "/usr/lib/libX11-xcb.so.1",
    "/usr/lib/libXi.so.6",
    "/usr/lib64/libX11.so.6",
    "/usr/lib64/libXcursor.so.1",
    "/usr/lib64/libX11-xcb.so.1",
    "/usr/lib64/libXi.so.6",
    "/lib/libX11.so.6",
    "/lib/libXcursor.so.1",
    "/lib/libX11-xcb.so.1",
    "/lib/libXi.so.6",
    "/lib64/libX11.so.6",
    "/lib64/libXcursor.so.1",
    "/lib64/libX11-xcb.so.1",
    "/lib64/libXi.so.6",
    "/run/current-system/sw/share/nix-ld/lib/libX11.so.6",
    "/run/current-system/sw/share/nix-ld/lib/libXcursor.so.1",
    "/run/current-system/sw/share/nix-ld/lib/libX11-xcb.so.1",
    "/run/current-system/sw/share/nix-ld/lib/libXi.so.6",
];

/// Nix store package name fragments → soname we need from that package.
#[cfg(target_os = "linux")]
const NIX_PKG_LIBS: &[(&str, &str)] = &[
    ("-libx11-", "libX11.so.6"),
    ("-libxcursor-", "libXcursor.so.1"),
    ("-libxi-", "libXi.so.6"),
    // libX11-xcb lives in libx11 package as well; also try dedicated names
    ("-libx11-", "libX11-xcb.so.1"),
];

#[cfg(target_os = "linux")]
fn push_unique(dirs: &mut Vec<String>, d: &str) {
    if d.is_empty() {
        return;
    }
    if !dirs.iter().any(|x| x == d) {
        dirs.push(d.to_string());
    }
}

/// Resolve a path with `readlink -f` (works with NixOS envfs where `exists()` can miss).
#[cfg(target_os = "linux")]
fn readlink_f(path: &str) -> Option<PathBuf> {
    let out = std::process::Command::new("readlink")
        .args(["-f", path])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let p = PathBuf::from(s);
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

/// `ls -d /nix/store/*-libxi-*/lib` style lookup (libXi is often missing from nix-ld).
#[cfg(target_os = "linux")]
fn nix_store_lib_dirs_for(pkg_frag: &str, soname: &str) -> Vec<String> {
    let pattern = format!("/nix/store/*{pkg_frag}*/lib");
    let out = std::process::Command::new("sh")
        .args(["-c", &format!("ls -d {pattern} 2>/dev/null")])
        .output();
    let Ok(out) = out else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let d = line.trim();
        if d.is_empty() {
            continue;
        }
        if std::path::Path::new(d).join(soname).exists() {
            push_unique(&mut dirs, d);
        }
    }
    dirs
}

/// Collect every directory that can satisfy winit's x11 dlopens.
#[cfg(target_os = "linux")]
fn dirs_with_x11_libs() -> Vec<String> {
    use std::path::Path;

    let mut dirs: Vec<String> = Vec::new();

    // 1) Direct FHS / nix-ld file checks + canonicalize
    for fhs in X11_FHS_FILES {
        let p = Path::new(fhs);
        let resolved = if p.exists() {
            p.canonicalize().ok().or_else(|| Some(p.to_path_buf()))
        } else {
            readlink_f(fhs)
        };
        let Some(resolved) = resolved else {
            continue;
        };
        if let Some(parent) = resolved.parent() {
            push_unique(&mut dirs, &parent.to_string_lossy());
        }
        if let Some(fhs_parent) = p.parent() {
            push_unique(&mut dirs, &fhs_parent.to_string_lossy());
        }
    }

    // 2) Explicit nix store package scan (fixes libXi missing from nix-ld)
    for (frag, soname) in NIX_PKG_LIBS {
        for d in nix_store_lib_dirs_for(frag, soname) {
            push_unique(&mut dirs, &d);
        }
    }

    // 3) NIX_LD_LIBRARY_PATH entries
    if let Ok(nix_ld) = std::env::var("NIX_LD_LIBRARY_PATH") {
        for part in nix_ld.split(':').filter(|p| !p.is_empty()) {
            push_unique(&mut dirs, part);
        }
    }
    push_unique(&mut dirs, "/run/current-system/sw/share/nix-ld/lib");

    dirs
}

#[cfg(target_os = "linux")]
fn build_safe_ld_library_path() -> String {
    let mut paths = dirs_with_x11_libs();
    // Mesa / EGL for glow
    for p in ["/run/opengl-driver/lib", "/run/current-system/sw/lib"] {
        if std::path::Path::new(p).is_dir() {
            push_unique(&mut paths, p);
        }
    }
    if let Ok(existing) = std::env::var("LD_LIBRARY_PATH") {
        for part in existing.split(':').filter(|p| !p.is_empty()) {
            if is_poison_ld_entry(part) {
                continue;
            }
            push_unique(&mut paths, part);
        }
    }
    paths.join(":")
}

#[cfg(target_os = "linux")]
fn ensure_clean_library_path() {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    // Inside steam-run: do not touch LD_LIBRARY_PATH (FHS runtime is correct).
    if std::env::var_os("BGC_INSIDE_STEAM_RUN").is_some() {
        return;
    }

    let safe = build_safe_ld_library_path();

    // Second process: still force the rebuilt path.
    if std::env::var_os("BGC_UI_LD_CLEAN").is_some() {
        unsafe {
            std::env::set_var("LD_LIBRARY_PATH", &safe);
        }
        return;
    }

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("bgc-ui: cannot re-exec to fix LD_LIBRARY_PATH ({e})");
            unsafe {
                std::env::set_var("LD_LIBRARY_PATH", &safe);
            }
            return;
        }
    };

    eprintln!("bgc-ui: re-exec with system X11 library path (NixOS/cargo safe)");
    let mut cmd = Command::new(&exe);
    cmd.args(std::env::args_os().skip(1));
    cmd.env("BGC_UI_LD_CLEAN", "1");
    cmd.env("LD_LIBRARY_PATH", &safe);
    if std::env::var_os("BGC_UI_WAYLAND").is_none() {
        cmd.env_remove("WAYLAND_DISPLAY");
        cmd.env_remove("WAYLAND_SOCKET");
    }
    let err = cmd.exec();
    eprintln!("bgc-ui: re-exec failed: {err}");
    unsafe {
        std::env::set_var("LD_LIBRARY_PATH", &safe);
    }
}

/// Fail early with the *exact* missing .so instead of winit's opaque message.
#[cfg(target_os = "linux")]
fn probe_x11_libs() -> Result<()> {
    use std::ffi::CString;

    for name in X11_LIBS {
        let cname = CString::new(*name).unwrap();
        // SAFETY: standard dlopen probe; handle leaked on purpose (process lifetime).
        let handle = unsafe { libc::dlopen(cname.as_ptr(), libc::RTLD_NOW) };
        if handle.is_null() {
            let msg = unsafe {
                let e = libc::dlerror();
                if e.is_null() {
                    "unknown dlopen error".into()
                } else {
                    std::ffi::CStr::from_ptr(e).to_string_lossy().into_owned()
                }
            };
            let ld = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
            let has_libxi_dir = ld.split(':').any(|d| {
                std::path::Path::new(d).join("libXi.so.6").exists()
            });
            anyhow::bail!(
                "cannot load {name}: {msg}\n\
                 libXi dir present in LD_LIBRARY_PATH: {has_libxi_dir}\n\
                 LD_LIBRARY_PATH={ld}\n\
                 Install X11 libs (NixOS: xorg.libX11 xorg.libXcursor xorg.libXi)"
            );
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn probe_x11_libs() -> Result<()> {
    Ok(())
}

fn native_options() -> eframe::NativeOptions {
    // Size/position always come from the live HS window when available.
    // Placeholder is only until the first attach — not a fixed target resolution.
    let geom = find_hearthstone_window();
    let (pos, size) = match geom {
        Some(g) => (
            egui::pos2(g.x as f32, g.y as f32),
            egui::vec2(g.width as f32, g.height as f32),
        ),
        None => (egui::pos2(64.0, 64.0), egui::vec2(960.0, 540.0)),
    };

    let mut opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BGC Overlay")
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_position(pos)
            .with_inner_size(size)
            // Clicks pass through until the cursor is over the HUD (toggled at runtime).
            .with_mouse_passthrough(true),
        ..Default::default()
    };

    // Force X11 backend (winit 0.30+) unless user opts into Wayland.
    #[cfg(all(target_os = "linux", not(target_arch = "wasm32")))]
    {
        if std::env::var_os("BGC_UI_WAYLAND").is_none() {
            opts.event_loop_builder = Some(Box::new(|builder| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }));
        }
    }

    opts
}

#[derive(Clone, Default)]
pub struct SharedState {
    pub live: Arc<Mutex<LiveState>>,
    pub status: Arc<Mutex<String>>,
    pub session: Arc<Mutex<String>>,
}

struct StateSink {
    shared: SharedState,
}

impl WatchSink for StateSink {
    fn on_event(&mut self, _event: &bgc_core::LogEvent, state: &LiveState) {
        if let Ok(mut g) = self.shared.live.lock() {
            *g = state.clone();
        }
    }

    fn on_tick(&mut self, state: &LiveState, _session_dir: &std::path::Path) {
        if let Ok(mut g) = self.shared.live.lock() {
            *g = state.clone();
        }
    }

    fn on_session(&mut self, session_dir: &std::path::Path) {
        let name = session_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        if let Ok(mut g) = self.shared.session.lock() {
            *g = name;
        }
    }

    fn on_status(&mut self, msg: &str) {
        if let Ok(mut g) = self.shared.status.lock() {
            *g = msg.to_string();
        }
    }
}

/// Start log watcher and open the in-game overlay (tracks the HS window).
pub fn run() -> Result<()> {
    prefer_display_backend();
    probe_x11_libs()?;
    ensure_card_data_loaded();
    let paths = discover_hs_paths()
        .paths
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No Hearthstone install found"))?;
    let _ = ensure_log_config(&paths.log_config);

    let shared = SharedState::default();
    {
        let mut s = shared.status.lock().unwrap();
        *s = format!("overlay · watching {}", paths.logs_dir.display());
    }

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_watch = stop.clone();
    let shared_watch = shared.clone();
    let logs_dir = paths.logs_dir.clone();

    thread::spawn(move || {
        let opts = WatchOptions {
            power_log: None,
            logs_dir: Some(logs_dir),
            from_start: false,
            poll_ms: 200,
        };
        let sink = StateSink {
            shared: shared_watch,
        };
        if let Err(e) = watch_power_log(opts, stop_watch, sink) {
            eprintln!("watch error: {e:#}");
        }
    });

    let stop_ui = stop.clone();
    eframe::run_native(
        "BGC Overlay",
        native_options(),
        Box::new(move |cc| {
            Ok(Box::new(BgcApp::new(cc, shared, stop_ui)) as Box<dyn eframe::App>)
        }),
    )
    .map_err(|e| {
        anyhow::anyhow!(
            "overlay failed to open ({e}).\n\
             Usually: poisoned AppImage LD_LIBRARY_PATH on NixOS. Try:\n\
               env -u LD_LIBRARY_PATH cargo run -p bgc-ui\n\
             DISPLAY={:?}  LD_LIBRARY_PATH={:?}",
            std::env::var_os("DISPLAY"),
            std::env::var("LD_LIBRARY_PATH")
                .ok()
                .map(|s| s.chars().take(160).collect::<String>())
        )
    })?;

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// Overlay against an explicit Power.log (demo / offline).
pub fn run_with_power_log(power_log: PathBuf, from_start: bool) -> Result<()> {
    prefer_display_backend();
    probe_x11_libs()?;
    ensure_card_data_loaded();
    let shared = SharedState::default();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_watch = stop.clone();
    let shared_watch = shared.clone();

    thread::spawn(move || {
        let opts = WatchOptions {
            power_log: Some(power_log),
            logs_dir: None,
            from_start,
            poll_ms: 200,
        };
        let sink = StateSink {
            shared: shared_watch,
        };
        let _ = watch_power_log(opts, stop_watch, sink);
    });

    let stop_ui = stop.clone();
    eframe::run_native(
        "BGC Overlay",
        native_options(),
        Box::new(move |cc| {
            Ok(Box::new(BgcApp::new(cc, shared, stop_ui)) as Box<dyn eframe::App>)
        }),
    )
    .map_err(|e| anyhow::anyhow!("overlay failed to open ({e})"))?;
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

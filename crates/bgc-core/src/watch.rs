//! Tail the **current** Hearthstone log session (not stale folders).

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};

use crate::discover::newest_log_session;
use crate::parser::{LogEvent, PowerParser};
use crate::runtime::runtime_status;
use crate::state::LiveState;

#[derive(Debug, Clone)]
pub struct WatchOptions {
    /// Explicit Power.log (skips session discovery)
    pub power_log: Option<PathBuf>,
    pub logs_dir: Option<PathBuf>,
    pub from_start: bool,
    pub poll_ms: u64,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            power_log: None,
            logs_dir: None,
            from_start: false,
            poll_ms: 250,
        }
    }
}

pub trait WatchSink {
    fn on_event(&mut self, event: &LogEvent, state: &LiveState);
    fn on_tick(&mut self, state: &LiveState, session_dir: &Path) {
        let _ = (state, session_dir);
    }
    fn on_session(&mut self, session_dir: &Path) {
        let _ = session_dir;
    }
    fn on_status(&mut self, msg: &str) {
        let _ = msg;
    }
}

struct Tail {
    path: PathBuf,
    reader: BufReader<File>,
}

impl Tail {
    fn open(path: &Path, from_start: bool) -> Result<Self> {
        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        if !from_start {
            file.seek(SeekFrom::End(0))?;
        }
        Ok(Self {
            path: path.to_path_buf(),
            reader: BufReader::new(file),
        })
    }

    fn reopen_from_start(&mut self) -> Result<()> {
        let file = File::open(&self.path)?;
        self.reader = BufReader::new(file);
        Ok(())
    }

    fn read_line(&mut self, buf: &mut String) -> Result<usize> {
        buf.clear();
        Ok(self.reader.read_line(buf)?)
    }

    fn fix_truncation(&mut self) -> Result<()> {
        if let Ok(meta) = std::fs::metadata(&self.path) {
            let pos = self.reader.stream_position().unwrap_or(0);
            if meta.len() < pos {
                self.reopen_from_start()?;
            }
        }
        Ok(())
    }

    fn open_at(path: &Path, offset: u64) -> Result<Self> {
        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        file.seek(SeekFrom::Start(offset))?;
        Ok(Self {
            path: path.to_path_buf(),
            reader: BufReader::new(file),
        })
    }
}

/// Byte offset of the last **GameState** `CREATE_GAME` (0 if none).
/// Ignores PowerTaskList mirrors — seeking those skips the real match start.
fn last_create_game_offset(path: &Path) -> Result<u64> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut offset = 0u64;
    let mut last = 0u64;
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        if line.contains("GameState.DebugPrintPower()") && line.contains("CREATE_GAME") {
            last = offset;
        }
        offset += n as u64;
    }
    Ok(last)
}

/// Parse Power.log from `start` to EOF into `state`, return a Tail parked at EOF.
fn catch_up_power(
    path: &Path,
    start: u64,
    parser: &mut PowerParser,
    state: &mut LiveState,
) -> Result<Tail> {
    let mut tail = Tail::open_at(path, start)?;
    let mut buf = String::new();
    while tail.read_line(&mut buf)? > 0 {
        state.lines_seen += 1;
        if let Some(event) = parser.parse_line(&buf) {
            state.apply(&event);
        }
    }
    Ok(tail)
}

/// HS hard-caps each log file at 10000KB then stops writing (message in Power.log).
/// We keep the file well under that so the feed never dies mid-session.
const HS_POWER_LOG_CAP_BYTES: u64 = 10_000 * 1024; // 10000KB as printed by HS
const POWER_LOG_ROTATE_BYTES: u64 = 3 * 1024 * 1024; // clear while still live
const POWER_LOG_STALE: Duration = Duration::from_secs(45);
const POWER_LOG_ROTATE_COOLDOWN: Duration = Duration::from_secs(10);
const KEEP_OLD_SESSIONS: usize = 2;

fn power_log_age(path: &Path) -> Option<Duration> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}

fn truncate_power_log(path: &Path) -> Result<()> {
    let f = OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("truncate-open {}", path.display()))?;
    f.set_len(0)?;
    let _ = f.sync_all();
    Ok(())
}

/// Delete old `Hearthstone_*` session folders; keep the newest few.
fn prune_old_log_sessions(logs_dir: &Path, keep: usize) {
    let Ok(rd) = fs::read_dir(logs_dir) else {
        return;
    };
    let mut sessions: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("Hearthstone_"))
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let t = e.metadata().ok()?.modified().ok()?;
            Some((t, e.path()))
        })
        .collect();
    sessions.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in sessions.into_iter().skip(keep) {
        let _ = fs::remove_dir_all(path);
    }
}

fn rotate_power_log_keep_state(
    path: &Path,
    power: &mut Option<Tail>,
    parser: &mut PowerParser,
    state: &mut LiveState,
    last_rotate: &mut Instant,
    sink: &mut impl WatchSink,
    reason: &str,
) -> Result<()> {
    // Match state stays in memory — only the on-disk ring buffer is cleared.
    truncate_power_log(path)?;
    *parser = PowerParser::new();
    *power = Some(Tail::open(path, false)?);
    *last_rotate = Instant::now();
    state.power_log_bytes = 0;
    state.power_log_stale = false;
    sink.on_status(&format!(
        "Power.log cleared ({reason}) — live match state kept in memory"
    ));
    sink.on_tick(state, path.parent().unwrap_or(path));
    Ok(())
}

/// Clear Power.log before HS hits 10000KB. Requires the feed to still be live.
fn maybe_rotate_power_log(
    path: &Path,
    power: &mut Option<Tail>,
    parser: &mut PowerParser,
    state: &mut LiveState,
    last_rotate: &mut Instant,
    last_power_progress: Instant,
    sink: &mut impl WatchSink,
) -> Result<()> {
    if !path.is_file() {
        state.power_log_bytes = 0;
        state.power_log_stale = false;
        return Ok(());
    }
    let meta = fs::metadata(path)?;
    let len = meta.len();
    state.power_log_bytes = len;
    let age = power_log_age(path).unwrap_or(Duration::ZERO);
    state.power_log_stale = age >= POWER_LOG_STALE;

    let still_live = last_power_progress.elapsed() < Duration::from_secs(15);
    let too_big = len >= POWER_LOG_ROTATE_BYTES;
    let near_hs_cap = len >= HS_POWER_LOG_CAP_BYTES * 8 / 10; // 80% of 10000KB
    if !(still_live && (too_big || near_hs_cap)) {
        return Ok(());
    }
    if last_rotate.elapsed() < POWER_LOG_ROTATE_COOLDOWN {
        return Ok(());
    }

    rotate_power_log_keep_state(
        path,
        power,
        parser,
        state,
        last_rotate,
        sink,
        &format!("{len} bytes / cap {HS_POWER_LOG_CAP_BYTES}"),
    )
}

/// Follow the newest Logs/Hearthstone_* session: LoadingScreen + Power.
pub fn watch_power_log<S: WatchSink>(
    opts: WatchOptions,
    stop: Arc<AtomicBool>,
    mut sink: S,
) -> Result<()> {
    let mut parser = PowerParser::new();
    let mut state = LiveState::default();
    let mut buf = String::new();

    // Explicit single-file mode (offline replay / follow)
    if let Some(ref power) = opts.power_log {
        sink.on_status(&format!("following explicit {}", power.display()));
        sink.on_session(power.parent().unwrap_or(power.as_path()));
        let start = if opts.from_start {
            0
        } else {
            last_create_game_offset(power)?
        };
        let mut tail = catch_up_power(power, start, &mut parser, &mut state)?;
        sink.on_status(&format!("Power.log catch-up → {}", state.summary_line()));
        while !stop.load(Ordering::Relaxed) {
            match tail.read_line(&mut buf)? {
                0 => {
                    thread::sleep(Duration::from_millis(opts.poll_ms));
                    tail.fix_truncation()?;
                    sink.on_tick(&state, power.parent().unwrap_or(power.as_path()));
                }
                _ => {
                    state.lines_seen += 1;
                    if let Some(event) = parser.parse_line(&buf) {
                        state.apply(&event);
                        sink.on_event(&event, &state);
                    }
                }
            }
        }
        return Ok(());
    }

    let logs_dir = opts
        .logs_dir
        .clone()
        .context("watch: logs_dir required unless --power-log is set")?;

    let mut session = newest_log_session(&logs_dir)
        .with_context(|| format!("no log sessions under {}", logs_dir.display()))?;
    prune_old_log_sessions(&logs_dir, KEEP_OLD_SESSIONS);
    sink.on_session(&session);
    let rt = runtime_status(&session);
    sink.on_status(&rt.summary_line(
        session
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?"),
    ));
    if !rt.is_live() {
        sink.on_status(
            "Hearthstone does not look active — showing last session idle. Start HS for a live feed.",
        );
    }

    let mut loading: Option<Tail> = None;
    let mut power: Option<Tail> = None;
    let mut gamenet: Option<Tail> = None;

    if session.join("LoadingScreen.log").is_file() {
        sink.on_status("LoadingScreen.log OK (mode detection)");
    } else {
        sink.on_status("Waiting for LoadingScreen.log…");
    }
    if session.join("GameNetLogger.log").is_file() {
        sink.on_status("GameNetLogger.log OK (queue detection)");
    }
    let power_path = session.join("Power.log");
    if power_path.is_file() {
        sink.on_status("Power.log present — catching up current match…");
    } else {
        sink.on_status(
            "No Power.log in THIS session yet — start a Battlegrounds match (lobby is not enough).",
        );
    }

    // Catch up LoadingScreen + GameNet silently (last mode/queue only — no spam from old lobbies)
    if session.join("LoadingScreen.log").is_file() {
        let mut t = Tail::open(&session.join("LoadingScreen.log"), true)?;
        while t.read_line(&mut buf)? > 0 {
            if let Some(mode) = parse_loading_screen_mode(&buf) {
                state.apply(&LogEvent::ScreenMode { mode });
            }
        }
        loading = Some(Tail::open(&session.join("LoadingScreen.log"), false)?);
    }
    if session.join("GameNetLogger.log").is_file() {
        let mut t = Tail::open(&session.join("GameNetLogger.log"), true)?;
        while t.read_line(&mut buf)? > 0 {
            if let Some(gs) = parse_find_game_state(&buf) {
                state.apply(&LogEvent::FindGameState { state: gs });
            }
        }
        gamenet = Some(Tail::open(&session.join("GameNetLogger.log"), false)?);
    }
    if power_path.is_file() {
        let start = if opts.from_start {
            0
        } else {
            last_create_game_offset(&power_path)?
        };
        parser = PowerParser::new();
        power = Some(catch_up_power(&power_path, start, &mut parser, &mut state)?);
    }
    // One status line after silent catch-up (no event spam from old lobby history)
    sink.on_status(&format!("caught up → {}", state.summary_line()));

    let mut last_status = Instant::now();
    let mut last_power_rotate = Instant::now() - POWER_LOG_ROTATE_COOLDOWN;
    let mut last_power_progress = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        if let Some(newest) = newest_log_session(&logs_dir) {
            if newest != session {
                session = newest;
                state = LiveState::default();
                sink.on_session(&session);
                sink.on_status(&format!(
                    "new session: {}",
                    session.file_name().and_then(|s| s.to_str()).unwrap_or("?")
                ));
                loading = open_if_exists(&session.join("LoadingScreen.log"), true)?;
                gamenet = open_if_exists(&session.join("GameNetLogger.log"), true)?;
                power = None;
                parser = PowerParser::new();
                let p = session.join("Power.log");
                if p.is_file() {
                    let start = if opts.from_start {
                        0
                    } else {
                        last_create_game_offset(&p).unwrap_or(0)
                    };
                    if let Ok(t) = catch_up_power(&p, start, &mut parser, &mut state) {
                        power = Some(t);
                        sink.on_status(&format!(
                            "new session catch-up → {}",
                            state.summary_line()
                        ));
                    }
                }
            }
        }

        if loading.is_none() {
            loading = open_if_exists(&session.join("LoadingScreen.log"), true)?;
            if loading.is_some() {
                sink.on_status("LoadingScreen.log appeared");
            }
        }
        if gamenet.is_none() {
            gamenet = open_if_exists(&session.join("GameNetLogger.log"), true)?;
            if gamenet.is_some() {
                sink.on_status("GameNetLogger.log appeared");
            }
        }
        if power.is_none() {
            let p = session.join("Power.log");
            if p.is_file() {
                let start = if opts.from_start {
                    0
                } else {
                    last_create_game_offset(&p).unwrap_or(0)
                };
                parser = PowerParser::new();
                match catch_up_power(&p, start, &mut parser, &mut state) {
                    Ok(t) => {
                        power = Some(t);
                        sink.on_status(&format!(
                            "Power.log appeared — catch-up → {}",
                            state.summary_line()
                        ));
                    }
                    Err(e) => sink.on_status(&format!("Power.log open failed: {e:#}")),
                }
            }
        }

        let power_path = session.join("Power.log");
        maybe_rotate_power_log(
            &power_path,
            &mut power,
            &mut parser,
            &mut state,
            &mut last_power_rotate,
            last_power_progress,
            &mut sink,
        )?;

        let mut progress = false;

        if let Some(ref mut t) = loading {
            match t.read_line(&mut buf)? {
                0 => t.fix_truncation()?,
                _ => {
                    progress = true;
                    state.lines_seen += 1;
                    apply_loading_screen_line(&buf, &mut state, &mut sink);
                }
            }
        }

        if let Some(ref mut t) = gamenet {
            match t.read_line(&mut buf)? {
                0 => t.fix_truncation()?,
                _ => {
                    progress = true;
                    state.lines_seen += 1;
                    apply_gamenet_line(&buf, &mut state, &mut sink);
                }
            }
        }

        let mut hit_hs_cap = false;
        if let Some(ref mut t) = power {
            match t.read_line(&mut buf)? {
                0 => t.fix_truncation()?,
                _ => {
                    progress = true;
                    last_power_progress = Instant::now();
                    state.power_log_stale = false;
                    state.lines_seen += 1;
                    // HS itself: "Truncating log, which has reached the size limit of 10000KB"
                    if buf.contains("size limit of 10000KB") {
                        hit_hs_cap = true;
                    } else if let Some(event) = parser.parse_line(&buf) {
                        state.apply(&event);
                        sink.on_event(&event, &state);
                    }
                }
            }
        }
        if hit_hs_cap && last_power_rotate.elapsed() >= POWER_LOG_ROTATE_COOLDOWN {
            let p = session.join("Power.log");
            let _ = rotate_power_log_keep_state(
                &p,
                &mut power,
                &mut parser,
                &mut state,
                &mut last_power_rotate,
                &mut sink,
                "HS hit 10000KB cap",
            );
        }

        if !progress {
            thread::sleep(Duration::from_millis(opts.poll_ms));
            if last_status.elapsed() > Duration::from_secs(5) {
                let rt = runtime_status(&session);
                sink.on_status(&format!(
                    "{} | mode={:?} queue={:?}",
                    rt.summary_line(
                        session
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("?")
                    ),
                    state.mode,
                    state.queue,
                ));
                last_status = std::time::Instant::now();
            }
            sink.on_tick(&state, &session);
        }
    }

    Ok(())
}

fn open_if_exists(path: &Path, from_start: bool) -> Result<Option<Tail>> {
    if path.is_file() {
        Ok(Some(Tail::open(path, from_start)?))
    } else {
        Ok(None)
    }
}

fn apply_loading_screen_line<S: WatchSink>(line: &str, state: &mut LiveState, sink: &mut S) {
    if let Some(mode) = parse_loading_screen_mode(line) {
        let ev = LogEvent::ScreenMode { mode };
        state.apply(&ev);
        sink.on_event(&ev, state);
    }
}

fn apply_gamenet_line<S: WatchSink>(line: &str, state: &mut LiveState, sink: &mut S) {
    if let Some(gs) = parse_find_game_state(line) {
        let ev = LogEvent::FindGameState { state: gs };
        state.apply(&ev);
        sink.on_event(&ev, state);
    }
}

fn parse_loading_screen_mode(line: &str) -> Option<String> {
    let key = "currMode=";
    let idx = line.find(key)?;
    let rest = &line[idx + key.len()..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '-' || c == ',')
        .unwrap_or(rest.len());
    let mode = rest[..end].trim().to_string();
    if mode.is_empty() {
        None
    } else {
        Some(mode)
    }
}

fn parse_find_game_state(line: &str) -> Option<String> {
    // GameMgr.ChangeFindGameState() - state: BNET_QUEUE_DELAYED, previous state: CLIENT_STARTED
    let key = "ChangeFindGameState() - state: ";
    let idx = line.find(key)?;
    let rest = &line[idx + key.len()..];
    let end = rest.find(',').unwrap_or(rest.len());
    let state = rest[..end].trim().to_string();
    if state.is_empty() {
        None
    } else {
        Some(state)
    }
}

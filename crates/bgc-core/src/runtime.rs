//! Detect whether Hearthstone appears to be running / writing logs.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct RuntimeStatus {
    pub hearthstone_process: bool,
    pub battlenet_process: bool,
    pub session_log_age: Option<Duration>,
    pub power_log_bytes: Option<u64>,
}

impl RuntimeStatus {
    pub fn summary_line(&self, session_name: &str) -> String {
        let hs = if self.hearthstone_process {
            "HS=running"
        } else {
            "HS=not running"
        };
        let age = match self.session_log_age {
            Some(d) if d < Duration::from_secs(90) => format!("logs fresh ({}s ago)", d.as_secs()),
            Some(d) if d < Duration::from_secs(3600) => {
                format!("logs stale ({}m ago)", d.as_secs() / 60)
            }
            Some(d) => format!("logs stale ({}h ago)", d.as_secs() / 3600),
            None => "logs=?".into(),
        };
        let power = self
            .power_log_bytes
            .map(|n| format!("Power.log {n} bytes"))
            .unwrap_or_else(|| "Power.log missing".into());
        format!("watching | {hs} | {session_name} | {age} | {power}")
    }

    pub fn is_live(&self) -> bool {
        self.hearthstone_process
            || self
                .session_log_age
                .is_some_and(|d| d < Duration::from_secs(90))
    }
}

pub fn process_running_substring(needle: &str) -> bool {
    // Prefer pgrep -f when available
    if let Ok(out) = Command::new("pgrep").args(["-af", needle]).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            return s.lines().any(|l| l.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()));
        }
    }
    false
}

pub fn hearthstone_running() -> bool {
    // Proton/Wine shows Hearthstone.exe
    process_running_substring("Hearthstone.exe")
        || process_running_substring("Hearthstone ")
}

pub fn battlenet_running() -> bool {
    process_running_substring("Battle.net.exe")
}

pub fn file_age(path: &Path) -> Option<Duration> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}

pub fn newest_mtime_in_dir(dir: &Path) -> Option<Duration> {
    let mut best: Option<SystemTime> = None;
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        if let Ok(m) = e.metadata() {
            if let Ok(t) = m.modified() {
                best = Some(match best {
                    Some(b) if b > t => b,
                    _ => t,
                });
            }
        }
    }
    best.and_then(|t| SystemTime::now().duration_since(t).ok())
}

pub fn runtime_status(session_dir: &Path) -> RuntimeStatus {
    let power = session_dir.join("Power.log");
    let power_log_bytes = std::fs::metadata(&power).ok().map(|m| m.len());
    let session_log_age = newest_mtime_in_dir(session_dir).or_else(|| file_age(&power));
    RuntimeStatus {
        hearthstone_process: hearthstone_running(),
        battlenet_process: battlenet_running(),
        session_log_age,
        power_log_bytes,
    }
}

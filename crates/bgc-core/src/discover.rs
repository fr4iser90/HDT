//! Locate Hearthstone install + Power.log under Linux / Proton / Wine.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walkdir::WalkDir;

/// Well-known Steam non-Steam shortcut / Battle.net helper used on this machine.
pub const KNOWN_STEAM_COMPAT_IDS: &[&str] = &[
    "2601819874", // Battle.net via Steam (user's current setup)
    "989080",     // Hearthstone on Steam (if ever used)
];

#[derive(Debug, Clone)]
pub struct HsPaths {
    pub prefix_root: PathBuf,
    pub install_dir: PathBuf,
    pub local_blizzard_hs: PathBuf,
    pub logs_dir: PathBuf,
    pub log_config: PathBuf,
    pub latest_power_log: Option<PathBuf>,
    /// Newest Logs/Hearthstone_* folder (may not contain Power.log yet)
    pub current_session: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct DiscoverReport {
    pub paths: Vec<HsPaths>,
    pub searched_roots: Vec<PathBuf>,
}

pub fn discover_hs_paths() -> DiscoverReport {
    let home = dirs_home();
    let mut searched = Vec::new();
    let mut found = Vec::new();

    let mut candidates: Vec<PathBuf> = Vec::new();

    // Steam compatdata prefixes
    for steam_root in [
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
    ] {
        let compat = steam_root.join("steamapps/compatdata");
        if compat.is_dir() {
            searched.push(compat.clone());
            for id in KNOWN_STEAM_COMPAT_IDS {
                candidates.push(compat.join(id).join("pfx"));
            }
            // Also scan for any Hearthstone install under compatdata (shallow)
            if let Ok(entries) = fs::read_dir(&compat) {
                for entry in entries.flatten() {
                    let pfx = entry.path().join("pfx");
                    if pfx.is_dir() {
                        candidates.push(pfx);
                    }
                }
            }
        }
    }

    // Lutris / manual wine prefixes
    candidates.push(home.join("Games/battlenet"));
    candidates.push(home.join(".wine"));

    candidates.sort();
    candidates.dedup();

    let mut seen_installs = std::collections::HashSet::new();
    for prefix in candidates {
        let Ok(canon) = fs::canonicalize(&prefix) else {
            continue;
        };
        if !seen_installs.insert(canon) {
            continue;
        }
        if let Some(paths) = probe_prefix(&prefix) {
            found.push(paths);
        }
    }

    // Prefer prefixes whose current session was touched most recently
    found.sort_by(|a, b| {
        session_activity(b).cmp(&session_activity(a))
    });

    DiscoverReport {
        paths: found,
        searched_roots: searched,
    }
}

fn session_activity(paths: &HsPaths) -> SystemTime {
    paths
        .current_session
        .as_ref()
        .and_then(|s| fs::metadata(s).ok())
        .and_then(|m| m.modified().ok())
        .or_else(|| {
            paths
                .latest_power_log
                .as_ref()
                .and_then(|p| fs::metadata(p).ok())
                .and_then(|m| m.modified().ok())
        })
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn probe_prefix(prefix: &Path) -> Option<HsPaths> {
    if !prefix.is_dir() {
        return None;
    }

    let install = find_hearthstone_install(prefix)?;
    let local = find_local_blizzard_hs(prefix).unwrap_or_else(|| {
        // Conventional Proton path even if not created yet
        prefix
            .join("drive_c/users/steamuser/AppData/Local/Blizzard/Hearthstone")
    });

    let logs_dir = install.join("Logs");
    let current_session = newest_log_session(&logs_dir);
    let latest_power_log = current_session
        .as_ref()
        .map(|s| s.join("Power.log"))
        .filter(|p| p.is_file());
    let log_config = local.join("log.config");

    Some(HsPaths {
        prefix_root: prefix.to_path_buf(),
        install_dir: install,
        local_blizzard_hs: local,
        logs_dir,
        log_config,
        latest_power_log,
        current_session,
    })
}

fn find_hearthstone_install(prefix: &Path) -> Option<PathBuf> {
    let candidates = [
        prefix.join("drive_c/Program Files (x86)/Hearthstone"),
        prefix.join("drive_c/Program Files/Hearthstone"),
    ];
    for c in candidates {
        if c.join("Hearthstone.exe").is_file() || c.is_dir() {
            // Prefer real install with exe
            if c.join("Hearthstone.exe").is_file() {
                return Some(c);
            }
        }
    }

    // Shallow walk fallback (depth-limited)
    for entry in WalkDir::new(prefix.join("drive_c"))
        .max_depth(6)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == "Hearthstone.exe" {
            return entry.path().parent().map(|p| p.to_path_buf());
        }
    }
    None
}

fn find_local_blizzard_hs(prefix: &Path) -> Option<PathBuf> {
    let users = prefix.join("drive_c/users");
    if !users.is_dir() {
        return None;
    }
    for user in fs::read_dir(users).ok()?.flatten() {
        let p = user
            .path()
            .join("AppData/Local/Blizzard/Hearthstone");
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

pub fn newest_power_log(logs_dir: &Path) -> Option<PathBuf> {
    if !logs_dir.is_dir() {
        return None;
    }
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for session in fs::read_dir(logs_dir).ok()?.flatten() {
        let power = session.path().join("Power.log");
        if !power.is_file() {
            continue;
        }
        let modified = fs::metadata(&power)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        // Prefer larger/recent files — empty error-only logs lose to real sessions
        let size = fs::metadata(&power).map(|m| m.len()).unwrap_or(0);
        let score_time = modified;
        match &best {
            Some((t, existing)) => {
                let existing_size = fs::metadata(existing).map(|m| m.len()).unwrap_or(0);
                if score_time > *t || (score_time == *t && size > existing_size) {
                    best = Some((score_time, power));
                }
            }
            None => best = Some((score_time, power)),
        }
    }
    best.map(|(_, p)| p)
}

/// Newest session directory under Logs/ (even if Power.log does not exist yet).
pub fn newest_log_session(logs_dir: &Path) -> Option<PathBuf> {
    if !logs_dir.is_dir() {
        return None;
    }
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for session in fs::read_dir(logs_dir).ok()?.flatten() {
        let path = session.path();
        if !path.is_dir() {
            continue;
        }
        let modified = fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        match &best {
            Some((t, _)) if *t >= modified => {}
            _ => best = Some((modified, path)),
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discovers_fake_prefix() {
        let dir = tempdir().unwrap();
        let prefix = dir.path().join("pfx");
        let install = prefix.join("drive_c/Program Files (x86)/Hearthstone");
        let logs = install.join("Logs/Hearthstone_test");
        let local = prefix.join("drive_c/users/steamuser/AppData/Local/Blizzard/Hearthstone");
        fs::create_dir_all(&logs).unwrap();
        fs::create_dir_all(&local).unwrap();
        fs::write(install.join("Hearthstone.exe"), b"").unwrap();
        fs::write(logs.join("Power.log"), b"D test\n").unwrap();

        let paths = probe_prefix(&prefix).expect("probe");
        assert!(paths.latest_power_log.as_ref().unwrap().ends_with("Power.log"));
        assert_eq!(paths.log_config, local.join("log.config"));
        assert!(paths.current_session.is_some());
    }
}

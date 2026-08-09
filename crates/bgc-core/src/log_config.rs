//! Ensure Hearthstone `log.config` enables Power.log.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Channels aligned with what working trackers enable (HDT-required set + Gameplay).
/// Written with Windows CRLF for Proton/Wine. Not copied from HDT source — public format.
///
/// `Verbose=False` on Power: Verbose=True fills the HS hard cap (~10000KB) within one
/// long session and then HS stops writing Power.log entirely.
pub const DEFAULT_LOG_CONFIG: &str = "\
[Power]\r\n\
LogLevel=1\r\n\
FilePrinting=True\r\n\
ConsolePrinting=False\r\n\
ScreenPrinting=False\r\n\
Verbose=False\r\n\
[LoadingScreen]\r\n\
LogLevel=1\r\n\
FilePrinting=True\r\n\
ConsolePrinting=False\r\n\
ScreenPrinting=False\r\n\
[Gameplay]\r\n\
LogLevel=1\r\n\
FilePrinting=True\r\n\
ConsolePrinting=False\r\n\
ScreenPrinting=False\r\n\
[Arena]\r\n\
LogLevel=1\r\n\
FilePrinting=True\r\n\
ConsolePrinting=False\r\n\
ScreenPrinting=False\r\n\
[Achievements]\r\n\
LogLevel=1\r\n\
FilePrinting=True\r\n\
ConsolePrinting=False\r\n\
ScreenPrinting=False\r\n\
[Bob]\r\n\
LogLevel=1\r\n\
FilePrinting=True\r\n\
ConsolePrinting=False\r\n\
ScreenPrinting=False\r\n\
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogConfigStatus {
    Created,
    Updated,
    AlreadyOk,
}

pub fn ensure_log_config(path: &Path) -> Result<LogConfigStatus> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }

    if path.is_file() {
        let existing = fs::read_to_string(path)?;
        if power_section_ok(&existing)
            && existing.contains("[LoadingScreen]")
            && existing.contains("[Achievements]")
        {
            return Ok(LogConfigStatus::AlreadyOk);
        }
        fs::write(path, DEFAULT_LOG_CONFIG)?;
        return Ok(LogConfigStatus::Updated);
    }

    fs::write(path, DEFAULT_LOG_CONFIG)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(LogConfigStatus::Created)
}

fn power_section_ok(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let Some(idx) = lower.find("[power]") else {
        return false;
    };
    let slice = lower[idx..].lines().take(10).collect::<Vec<_>>().join("\n");
    // Verbose=True fills the 10000KB HS cap too fast — reject it so we rewrite.
    slice.contains("fileprinting=true")
        && slice.contains("loglevel=1")
        && slice.contains("verbose=false")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("log.config");
        assert_eq!(ensure_log_config(&path).unwrap(), LogConfigStatus::Created);
        assert_eq!(ensure_log_config(&path).unwrap(), LogConfigStatus::AlreadyOk);
        let s = fs::read_to_string(&path).unwrap();
        assert!(s.contains("Verbose=False"));
        assert!(s.contains('\r'));
    }

    #[test]
    fn rewrites_verbose_true() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("log.config");
        fs::write(
            &path,
            "[Power]\r\nLogLevel=1\r\nFilePrinting=True\r\nVerbose=True\r\n[LoadingScreen]\r\n[Achievements]\r\n",
        )
        .unwrap();
        assert_eq!(ensure_log_config(&path).unwrap(), LogConfigStatus::Updated);
        let s = fs::read_to_string(&path).unwrap();
        assert!(s.contains("Verbose=False"));
    }
}

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use bgc_core::parser::LogEvent;
use bgc_core::state::LiveState;
use bgc_core::{
    card_db_count, card_def, discover_hs_paths, ensure_card_data_loaded, ensure_log_config,
    sync_card_db, watch_power_log, DiscoverReport, HsPaths, WatchOptions, WatchSink,
};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "bgc",
    about = "Battlegrounds Companion — Linux-first HS BG detection spike",
    version
)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Find Hearthstone Proton/Wine paths and Power.log
    Discover,
    /// Write/repair log.config so Power.log is enabled (restart HS afterwards)
    Setup {
        /// Optional override for log.config path
        #[arg(long)]
        log_config: Option<PathBuf>,
    },
    /// Tail Power.log and print live MVP state (turn/gold/hp/tavern/…)
    Watch {
        /// Read existing file from the beginning (useful for offline logs)
        #[arg(long)]
        from_start: bool,
        /// Print every parsed event, not only state changes of interest
        #[arg(long)]
        verbose: bool,
        /// Explicit Power.log path
        #[arg(long)]
        power_log: Option<PathBuf>,
        /// Dump state as JSON lines instead of human summary
        #[arg(long)]
        json: bool,
    },
    /// One-shot: discover + setup + print what to do next
    Doctor,
    /// Open the in-game overlay (transparent HUD locked to the HS window)
    Ui {
        #[arg(long)]
        power_log: Option<PathBuf>,
        #[arg(long)]
        from_start: bool,
    },
    /// Card database (HearthstoneJSON → local BG index)
    Cards {
        #[command(subcommand)]
        cmd: CardsCmd,
    },
}

#[derive(Subcommand, Debug)]
enum CardsCmd {
    /// Show whether the local BG card index is loaded
    Status,
    /// Download latest cards.json and rebuild `data/cards/bg_enUS.json`
    Sync {
        #[arg(long, default_value = "data/cards")]
        out_dir: PathBuf,
    },
    /// Look up a card id (e.g. BG35_814)
    Get {
        id: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Discover => cmd_discover(),
        Commands::Setup { log_config } => cmd_setup(log_config),
        Commands::Watch {
            from_start,
            verbose,
            power_log,
            json,
        } => cmd_watch(from_start, verbose, power_log, json),
        Commands::Doctor => cmd_doctor(),
        Commands::Ui {
            power_log,
            from_start,
        } => {
            if let Some(p) = power_log {
                bgc_ui::run_with_power_log(p, from_start)
            } else {
                bgc_ui::run()
            }
        }
        Commands::Cards { cmd } => cmd_cards(cmd),
    }
}

fn cmd_cards(cmd: CardsCmd) -> Result<()> {
    match cmd {
        CardsCmd::Status => {
            ensure_card_data_loaded();
            let n = card_db_count();
            if n == 0 {
                println!("Card DB empty. Run: cargo run -p bgc-cli -- cards sync");
            } else {
                println!("Card DB loaded: {n} Battlegrounds cards (data/cards/bg_enUS.json)");
            }
            Ok(())
        }
        CardsCmd::Sync { out_dir } => {
            println!("Downloading HearthstoneJSON latest cards.json…");
            let path = sync_card_db(&out_dir)?;
            println!(
                "Wrote {} ({} BG cards)",
                path.display(),
                card_db_count()
            );
            Ok(())
        }
        CardsCmd::Get { id } => {
            ensure_card_data_loaded();
            match card_def(&id) {
                Some(c) => {
                    println!("{}", serde_json::to_string_pretty(&c)?);
                }
                None => {
                    bail!("Unknown card id '{id}' (DB has {} cards)", card_db_count());
                }
            }
            Ok(())
        }
    }
}

fn cmd_discover() -> Result<()> {
    let report = discover_hs_paths();
    print_discover(&report);
    if report.paths.is_empty() {
        bail!("No Hearthstone install found under Steam compatdata / Games/battlenet / .wine");
    }
    Ok(())
}

fn print_discover(report: &DiscoverReport) {
    println!("Searched roots:");
    for r in &report.searched_roots {
        println!("  - {}", r.display());
    }
    println!();
    if report.paths.is_empty() {
        println!("No installs found.");
        return;
    }
    for (i, p) in report.paths.iter().enumerate() {
        println!("[{i}] prefix:      {}", p.prefix_root.display());
        println!("    install:     {}", p.install_dir.display());
        println!("    local data:  {}", p.local_blizzard_hs.display());
        println!("    log.config:  {}", p.log_config.display());
        println!("    logs dir:    {}", p.logs_dir.display());
        match &p.current_session {
            Some(s) => println!(
                "    session:     {}",
                s.file_name().and_then(|n| n.to_str()).unwrap_or("?")
            ),
            None => println!("    session:     (none)"),
        }
        match &p.latest_power_log {
            Some(pl) => {
                let size = std::fs::metadata(pl).map(|m| m.len()).unwrap_or(0);
                println!("    Power.log:   {} ({size} bytes) [current session]", pl.display());
            }
            None => println!(
                "    Power.log:   (none in current session — start a BG match; lobby is not enough)"
            ),
        }
        let loading = p
            .current_session
            .as_ref()
            .map(|s| s.join("LoadingScreen.log"))
            .filter(|f| f.is_file());
        if let Some(ls) = loading {
            println!("    LoadingScreen: {} (OK)", ls.display());
        }
        println!();
    }
}

fn primary_paths() -> Result<HsPaths> {
    let report = discover_hs_paths();
    report
        .paths
        .into_iter()
        .next()
        .context("No Hearthstone install found. Start HS once via your Battle.net/Proton setup, then retry.")
}

fn cmd_setup(log_config: Option<PathBuf>) -> Result<()> {
    if let Some(path) = log_config {
        let status = ensure_log_config(&path)?;
        println!("{status:?}: {}", path.display());
    } else {
        let report = discover_hs_paths();
        if report.paths.is_empty() {
            bail!("No Hearthstone install found");
        }
        let mut seen = std::collections::HashSet::new();
        for p in report.paths {
            let key = std::fs::canonicalize(&p.log_config)
                .unwrap_or_else(|_| p.log_config.clone());
            if !seen.insert(key) {
                continue;
            }
            let status = ensure_log_config(&p.log_config)?;
            println!("{status:?}: {}", p.log_config.display());
        }
    }
    println!();
    println!("IMPORTANT: Fully quit Hearthstone (and ideally Battle.net), then start again");
    println!("so a NEW Logs/Hearthstone_*/Power.log is created with GameState lines.");
    Ok(())
}

fn cmd_doctor() -> Result<()> {
    println!("=== bgc doctor ===\n");
    let report = discover_hs_paths();
    print_discover(&report);
    let paths = report
        .paths
        .first()
        .context("No Hearthstone install found")?;

    let status = ensure_log_config(&paths.log_config)?;
    println!("log.config: {status:?} → {}", paths.log_config.display());

    match &paths.latest_power_log {
        Some(pl) => {
            let size = std::fs::metadata(pl)?.len();
            println!("current-session Power.log: {} bytes", size);
            if size < 2048 {
                println!("NOTE: still small — enter an actual match; GameState lines appear in-game.");
            }
        }
        None => {
            println!("No Power.log in current session.");
            println!("LoadingScreen logging works → log.config is read.");
            println!("Start a Battlegrounds MATCH (not just the BACON lobby), then re-check.");
        }
    }

    println!(
        "\nNext:\n  cargo run -p bgc-cli -- watch --verbose\n  → queue into a live Battlegrounds game\n"
    );
    Ok(())
}

struct CliSink {
    verbose: bool,
    json: bool,
    last_summary: String,
    last_print: Instant,
}

impl WatchSink for CliSink {
    fn on_event(&mut self, event: &LogEvent, state: &LiveState) {
        if self.verbose {
            println!("event: {event:?}");
        }
        if self.json {
            if let Ok(s) = serde_json::to_string(state) {
                println!("{s}");
            }
            return;
        }
        let summary = state.summary_line();
        if summary != self.last_summary {
            self.last_summary = summary.clone();
            println!("{summary}");
            self.last_print = Instant::now();
        }
    }

    fn on_tick(&mut self, state: &LiveState, session_dir: &std::path::Path) {
        if self.json || self.verbose {
            return;
        }
        // Heartbeat at most every 30s when nothing meaningful changed
        if self.last_print.elapsed() > Duration::from_secs(30) {
            println!(
                "(idle {}) {}",
                session_dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?"),
                state.summary_line()
            );
            self.last_print = Instant::now();
        }
    }

    fn on_session(&mut self, session_dir: &std::path::Path) {
        println!("→ session {}", session_dir.display());
        self.last_summary.clear();
    }

    fn on_status(&mut self, msg: &str) {
        println!("status: {msg}");
        self.last_print = Instant::now();
    }
}

fn cmd_watch(
    from_start: bool,
    verbose: bool,
    power_log: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let paths = primary_paths()?;
    ensure_log_config(&paths.log_config)?;
    ensure_card_data_loaded();
    if card_db_count() > 0 {
        println!("cards:   {} BG defs loaded", card_db_count());
    } else {
        println!("cards:   (none — run `bgc cards sync`)");
    }

    let opts = WatchOptions {
        power_log: None, // always pick newest under logs_dir; allow wait
        logs_dir: Some(paths.logs_dir.clone()),
        from_start,
        poll_ms: 200,
    };

    // If user passed an explicit file, use it
    let opts = if let Some(p) = power_log {
        WatchOptions {
            power_log: Some(p),
            ..opts
        }
    } else {
        opts
    };

    println!("Battlegrounds Companion — watch mode");
    println!("install: {}", paths.install_dir.display());
    println!("logs:    {}", paths.logs_dir.display());
    println!("Ctrl+C to stop\n");

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        ctrlc_set(stop)?;
    }

    let sink = CliSink {
        verbose,
        json,
        last_summary: String::new(),
        last_print: Instant::now(),
    };

    watch_power_log(opts, stop, sink)?;
    Ok(())
}

fn ctrlc_set(stop: Arc<AtomicBool>) -> Result<()> {
    // Avoid extra dep: use simple signal via ctrlc if available — use libc-free approach
    // with `ctrlc` crate would be nicer; keep dependency light using signal_hook-less:
    ctrlc::set_handler(move || {
        stop.store(true, Ordering::Relaxed);
    })
    .context("install Ctrl+C handler")?;
    Ok(())
}
